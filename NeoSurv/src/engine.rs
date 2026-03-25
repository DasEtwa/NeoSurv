use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use glam::{IVec3, Vec3};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::KeyCode,
    window::{CursorGrabMode, Window, WindowAttributes, WindowId},
};

use crate::{
    chat::ChatState,
    commands::{CommandContext, CommandRegistry},
    config::AppConfig,
    gameplay::CombatState,
    hud,
    input::handler::InputHandler,
    menu::{MenuCommand, StartMenuState},
    player::Player,
    renderer::{
        CameraMatrices, Renderer,
        backend_trait::{Backend, ClearColor},
    },
    world::{
        save::WorldSaveManager,
        state::WorldRuntimeState,
        voxel::{ChunkMeshUpdate, VoxelWorld},
    },
};

pub(crate) fn run(config: AppConfig) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = EngineApp::new(config);
    event_loop.run_app(&mut app)?;
    Ok(())
}

const MAX_FRAME_DELTA: Duration = Duration::from_millis(100);
const CHUNK_VISIBILITY_RADIUS: u32 = 4;
const MAX_MESH_UPDATES_TO_RENDERER_PER_FRAME: usize = 64;
const CHUNK_UPLOAD_BUDGET_BYTES_PER_FRAME: usize = 2 * 1024 * 1024;
const STREAM_TELEMETRY_INTERVAL_FRAMES: u64 = 120;
const WORLD_BORDER_SIZE_BLOCKS: f32 = 250.0;
const WORLD_BORDER_HALF_EXTENT: f32 = WORLD_BORDER_SIZE_BLOCKS * 0.5;
const WORLD_BORDER_WALKABLE_HALF_EXTENT: f32 = WORLD_BORDER_HALF_EXTENT - 1.0 - 0.25;
const SPAWN_SURFACE_SEARCH_TOP_Y: i32 = 96;
const SPAWN_SURFACE_SEARCH_BOTTOM_Y: i32 = -32;

fn default_voxel_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|parallelism| parallelism.get().saturating_sub(1).max(1))
        .unwrap_or(1)
        .min(6)
}

fn find_surface_height_in_world(voxel_world: &VoxelWorld, x: i32, z: i32) -> Option<i32> {
    for y in (SPAWN_SURFACE_SEARCH_BOTTOM_Y..=SPAWN_SURFACE_SEARCH_TOP_Y).rev() {
        if voxel_world.block_at_world(IVec3::new(x, y, z)).is_some() {
            return Some(y);
        }
    }

    None
}

struct EngineApp {
    config: AppConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    world_saves: WorldSaveManager,
    runtime_state: WorldRuntimeState,
    cached_world_meshes: Vec<crate::renderer::StaticModelMesh>,
    world_meshes_dirty: bool,
    input: InputHandler,
    voxel_world: VoxelWorld,
    player: Player,
    start: Instant,
    last_frame: Instant,
    frame_index: u64,
    window_occluded: bool,
    stream_uploaded_chunks_since_log: u64,
    stream_uploaded_bytes_since_log: u64,
    stream_drawn_chunks_since_log: u64,
    last_stream_telemetry_frame: u64,
    mouse_captured: bool,
    menu_open: bool,
    primary_fire_requested: bool,
    start_menu: StartMenuState,
    combat: CombatState,
    chat: ChatState,
    commands: CommandRegistry,
}

impl EngineApp {
    fn new(config: AppConfig) -> Self {
        let world_saves = WorldSaveManager::load_or_default(env!("CARGO_MANIFEST_DIR"));
        let world_seed = world_saves
            .load_selected_world()
            .map(|world| world.seed)
            .unwrap_or(0xC0FF_EE42);
        let mut app = Self {
            player: Player::new(config.input.mouse_sensitivity),
            config,
            window: None,
            renderer: None,
            world_saves,
            runtime_state: WorldRuntimeState::new_singleplayer(world_seed),
            cached_world_meshes: Vec::new(),
            world_meshes_dirty: true,
            input: InputHandler::default(),
            voxel_world: VoxelWorld::new(world_seed, default_voxel_worker_count()),
            start: Instant::now(),
            last_frame: Instant::now(),
            frame_index: 0,
            window_occluded: false,
            stream_uploaded_chunks_since_log: 0,
            stream_uploaded_bytes_since_log: 0,
            stream_drawn_chunks_since_log: 0,
            last_stream_telemetry_frame: 0,
            mouse_captured: false,
            menu_open: false,
            primary_fire_requested: false,
            start_menu: StartMenuState::new(),
            combat: CombatState::new(),
            chat: ChatState::new(),
            commands: CommandRegistry::new(),
        };
        app.reset_run_state();
        app.restore_selected_world_runtime();
        app
    }

    fn reset_run_state(&mut self) {
        let now = Instant::now();
        self.input.clear();
        self.player.reset();
        self.start = now;
        self.last_frame = now;
        self.frame_index = 0;
        self.window_occluded = false;
        self.stream_uploaded_chunks_since_log = 0;
        self.stream_uploaded_bytes_since_log = 0;
        self.stream_drawn_chunks_since_log = 0;
        self.last_stream_telemetry_frame = 0;
        self.mouse_captured = false;
        self.menu_open = true;
        self.primary_fire_requested = false;
        self.combat.reset();
        self.chat.close();
        self.world_meshes_dirty = true;
    }

    fn restore_selected_world_runtime(&mut self) {
        let Some(world) = self.world_saves.load_selected_world() else {
            return;
        };

        self.voxel_world = VoxelWorld::new(world.seed, default_voxel_worker_count());
        self.player.reset();
        self.runtime_state = world.runtime_state;
        self.runtime_state.terrain_snap(world.seed);
        self.combat.reset();
        self.world_meshes_dirty = true;

        if let Some(saved_pose) = self.runtime_state.local_player_pose() {
            self.player.restore_saved_pose(saved_pose);
        }

        tracing::info!(
            world = world.name,
            seed = world.seed,
            "selected world restored"
        );
    }

    fn save_selected_world_runtime(&mut self) {
        self.runtime_state
            .sync_local_player_pose(self.player.saved_pose());
        self.world_saves.save_selected_world(&self.runtime_state);
    }

    fn world_occlusion_distance(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<f32> {
        let voxel_distance = self
            .voxel_world
            .raycast_solid_distance(origin, direction, max_distance);
        let prop_distance = self
            .runtime_state
            .ray_static_prop_distance(origin, direction, max_distance);

        match (voxel_distance, prop_distance) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(distance), None) | (None, Some(distance)) => Some(distance),
            (None, None) => None,
        }
    }

    fn handle_menu_command(&mut self, command: MenuCommand, event_loop: &ActiveEventLoop) {
        match command {
            MenuCommand::PlaySelectedWorld => {
                self.restore_selected_world_runtime();
                self.set_mouse_captured(true);
            }
            MenuCommand::SelectPreviousWorld => {
                self.world_saves.select_previous_world();
                self.restore_selected_world_runtime();
            }
            MenuCommand::SelectNextWorld => {
                self.world_saves.select_next_world();
                self.restore_selected_world_runtime();
            }
            MenuCommand::CreateWorld => {
                if self.world_saves.create_world().is_some() {
                    self.restore_selected_world_runtime();
                }
            }
            MenuCommand::SaveWorld => {
                self.save_selected_world_runtime();
            }
            MenuCommand::Quit => {
                self.save_selected_world_runtime();
                event_loop.exit();
            }
        }
    }

    fn can_render(&self) -> bool {
        if self.window_occluded {
            return false;
        }

        self.window
            .as_ref()
            .map(|window| {
                let size = window.inner_size();
                size.width > 0 && size.height > 0
            })
            .unwrap_or(false)
    }

    fn clamp_to_world_border(&mut self) {
        self.player
            .clamp_to_world_border(WORLD_BORDER_WALKABLE_HALF_EXTENT);
    }

    fn set_mouse_captured(&mut self, captured: bool) {
        if self.mouse_captured == captured {
            self.menu_open = !captured;
            return;
        }

        let Some(window) = self.window.as_ref() else {
            self.mouse_captured = captured;
            self.menu_open = !captured;
            return;
        };

        if captured {
            let lock_result = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));

            match lock_result {
                Ok(()) => {
                    window.set_cursor_visible(false);
                    self.mouse_captured = true;
                    self.menu_open = false;
                    tracing::info!("mouse captured (fps control enabled)");
                }
                Err(err) => {
                    window.set_cursor_visible(true);
                    self.mouse_captured = false;
                    self.menu_open = true;
                    tracing::warn!(?err, "failed to capture mouse cursor");
                }
            }
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                tracing::debug!(?err, "failed to release cursor grab cleanly");
            }
            window.set_cursor_visible(true);
            self.mouse_captured = false;
            self.menu_open = true;
            tracing::info!("menu mode active (mouse released)");
        }

        let _ = self.input.take_mouse_delta();
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_none() || !self.can_render() {
            self.last_frame = Instant::now();
            return;
        }

        let now = Instant::now();
        let raw_dt = now.duration_since(self.last_frame);
        self.last_frame = now;

        let dt = raw_dt.min(MAX_FRAME_DELTA);
        if raw_dt > MAX_FRAME_DELTA {
            tracing::debug!(
                raw_dt_ms = raw_dt.as_secs_f64() * 1_000.0,
                clamped_dt_ms = dt.as_secs_f64() * 1_000.0,
                "frame delta clamped after stall"
            );
        }

        let dt_seconds = dt.as_secs_f32();
        let daylight = self.runtime_state.time_of_day.daylight_factor() as f64;
        let clear = ClearColor {
            r: 0.07 + 0.42 * daylight,
            g: 0.10 + 0.56 * daylight,
            b: 0.16 + 0.72 * daylight,
            a: 1.0,
        };
        self.combat.tick_effects(dt_seconds);

        if self.menu_open {
            if self.input.consume_key_press(KeyCode::ArrowUp)
                || self.input.consume_key_press(KeyCode::KeyW)
            {
                self.start_menu.move_selection_up();
            }
            if self.input.consume_key_press(KeyCode::ArrowDown)
                || self.input.consume_key_press(KeyCode::KeyS)
            {
                self.start_menu.move_selection_down();
            }
            if self.input.consume_key_press(KeyCode::ArrowLeft)
                || self.input.consume_key_press(KeyCode::KeyA)
            {
                self.handle_menu_command(MenuCommand::SelectPreviousWorld, event_loop);
            }
            if self.input.consume_key_press(KeyCode::ArrowRight)
                || self.input.consume_key_press(KeyCode::KeyD)
            {
                self.handle_menu_command(MenuCommand::SelectNextWorld, event_loop);
            }
            if self.input.consume_key_press(KeyCode::Enter) {
                self.handle_menu_command(self.start_menu.activate_selected(), event_loop);
            }
        }

        let typed_text = self.input.take_typed_text();
        if self.chat.is_open() {
            self.chat.append_text(&typed_text);
            if self.input.consume_key_press(KeyCode::Backspace) {
                self.chat.backspace();
            }
            if self.input.consume_key_press(KeyCode::Enter)
                && let Some(submitted) = self.chat.submit()
            {
                let mut context = CommandContext {
                    world: &mut self.runtime_state,
                    player_position: self.player.position(),
                    player_forward: self.player.forward(),
                };
                let outcome = self.commands.execute(&submitted, &mut context);
                for line in outcome.lines {
                    self.chat.push_system_line(line);
                }
                self.world_meshes_dirty = true;
                if outcome.save_requested {
                    self.save_selected_world_runtime();
                }
                if outcome.load_requested {
                    self.restore_selected_world_runtime();
                }
            }
            if self.input.consume_key_press(KeyCode::Escape) {
                self.chat.close();
            }
        } else if !self.menu_open {
            if self.input.consume_key_press(KeyCode::KeyT) {
                self.chat.open();
            } else if self.input.consume_key_press(KeyCode::Slash) {
                self.chat.open_with_slash();
            }
        }

        let simulation_paused = self.menu_open || self.chat.is_open();

        if !simulation_paused && self.input.consume_key_press(KeyCode::Digit1) {
            self.runtime_state.select_weapon_slot(0);
        }
        if !simulation_paused && self.input.consume_key_press(KeyCode::Digit2) {
            self.runtime_state.select_weapon_slot(1);
        }
        if self.input.consume_key_press(KeyCode::Digit3)
            && !simulation_paused
            && let Some(line) = self.runtime_state.use_heal_item()
        {
            self.chat.push_system_line(line);
        }
        if self.input.consume_key_press(KeyCode::Digit4) && !simulation_paused {
            if self.runtime_state.consume_throwable() {
                let origin = self.player.position();
                let direction = self.player.forward().normalize_or_zero();
                self.combat.fire_weapon(
                    crate::inventory::ItemId::Grenade,
                    &mut self.runtime_state.enemies,
                    origin,
                    direction,
                    None,
                );
                self.chat.push_system_line("THREW GRENADE");
            } else {
                self.chat.push_system_line("NO GRENADES");
            }
        }
        let voxel_world = &self.voxel_world;
        self.player.update_look_and_move(
            &mut self.input,
            self.mouse_captured,
            simulation_paused,
            dt_seconds,
            |world| voxel_world.block_at_world(world).is_some(),
        );
        self.clamp_to_world_border();

        let voxel_report = self.voxel_world.tick(self.player.position());
        let _ = self
            .player
            .try_align_spawn_to_surface(|world| self.voxel_world.block_at_world(world).is_some());
        self.player.apply_jump_and_gravity(
            &mut self.input,
            simulation_paused,
            dt_seconds,
            |world| self.voxel_world.block_at_world(world).is_some(),
        );
        self.clamp_to_world_border();
        if !simulation_paused {
            self.runtime_state.time_of_day.advance(dt_seconds);
            self.runtime_state
                .tick_spawners(dt_seconds, self.player.position(), |x, z| {
                    find_surface_height_in_world(&self.voxel_world, x, z)
                });
            self.runtime_state
                .tick_enemy_ai(dt_seconds, self.player.position(), |x, z| {
                    find_surface_height_in_world(&self.voxel_world, x, z)
                });
        }
        self.combat
            .tick_projectiles(dt_seconds, &mut self.runtime_state.enemies, |world| {
                self.voxel_world.block_at_world(world).is_some()
            });

        if self.primary_fire_requested && self.mouse_captured && !simulation_paused {
            if let Some(item_id) = self.runtime_state.selected_weapon_item() {
                let origin = self.player.position();
                let direction = self.player.forward().normalize_or_zero();
                let world_blocker_distance = self
                    .combat
                    .hitscan_range_for_item(item_id)
                    .and_then(|range| self.world_occlusion_distance(origin, direction, range));
                self.combat.fire_weapon(
                    item_id,
                    &mut self.runtime_state.enemies,
                    origin,
                    direction,
                    world_blocker_distance,
                );
            }
        }
        if self.mouse_captured
            && !simulation_paused
            && self.input.consume_key_press(KeyCode::KeyE)
            && let Some(line) = {
                let origin = self.player.position();
                let direction = self.player.forward();
                let blocker_distance = self.world_occlusion_distance(origin, direction, 4.5);
                self.runtime_state
                    .try_open_chest(origin, direction, blocker_distance)
            }
        {
            self.chat.push_system_line(line);
            self.world_meshes_dirty = true;
        }
        if self.mouse_captured && !simulation_paused && self.input.consume_key_press(KeyCode::KeyQ)
        {
            if self.runtime_state.consume_throwable() {
                let origin = self.player.position();
                let direction = self.player.forward().normalize_or_zero();
                self.combat.fire_weapon(
                    crate::inventory::ItemId::Grenade,
                    &mut self.runtime_state.enemies,
                    origin,
                    direction,
                    None,
                );
                self.chat.push_system_line("THREW GRENADE");
            } else {
                self.chat.push_system_line("NO GRENADES");
            }
        }
        if self.mouse_captured && !simulation_paused && self.input.consume_key_press(KeyCode::KeyF)
        {
            if let Some(item_id) = self.runtime_state.selected_weapon_item() {
                let origin = self.player.position();
                let direction = self.player.forward().normalize_or_zero();
                let world_blocker_distance = self
                    .combat
                    .hitscan_range_for_item(item_id)
                    .and_then(|range| self.world_occlusion_distance(origin, direction, range));
                self.combat.fire_weapon(
                    item_id,
                    &mut self.runtime_state.enemies,
                    origin,
                    direction,
                    world_blocker_distance,
                );
            }
        }
        self.primary_fire_requested = false;

        if self.mouse_captured && !simulation_paused && self.input.consume_key_press(KeyCode::KeyR)
        {
            self.chat.push_system_line(format!(
                "TIME {} {:.2}",
                self.runtime_state.time_of_day.label(),
                self.runtime_state.time_of_day.normalized_time
            ));
        }

        if self.mouse_captured && !simulation_paused && self.input.consume_key_press(KeyCode::KeyH)
        {
            if let Some(line) = self.runtime_state.use_heal_item() {
                self.chat.push_system_line(line);
            }
        }

        if voxel_report.completed > 0
            || voxel_report.mesh_updates_queued > 0
            || (self.frame_index.is_multiple_of(240) && voxel_report.pending_chunks > 0)
        {
            tracing::debug!(
                voxel_requested = voxel_report.requested,
                voxel_completed = voxel_report.completed,
                voxel_loaded = voxel_report.loaded_chunks,
                voxel_pending = voxel_report.pending_chunks,
                voxel_mesh_updates = voxel_report.mesh_updates_queued,
                "voxel chunk generation update"
            );
        }

        let chunk_updates = self
            .voxel_world
            .drain_mesh_updates(MAX_MESH_UPDATES_TO_RENDERER_PER_FRAME);
        let runtime_mesh_backlog = self.voxel_world.pending_mesh_update_count();

        let aspect_ratio = self
            .window
            .as_ref()
            .map(|window| {
                let size = window.inner_size();
                size.width.max(1) as f32 / size.height.max(1) as f32
            })
            .unwrap_or(1.0);

        let camera_matrices = CameraMatrices {
            view: self.player.view_matrix(),
            projection: self.player.projection_matrix(aspect_ratio),
        };
        let dynamic_world_meshes = self
            .combat
            .build_world_static_meshes(&self.runtime_state.enemies);
        let viewmodel_meshes = if self.menu_open {
            self.start_menu.build_meshes(
                self.player.camera(),
                &self.world_saves.selected_world_name(),
            )
        } else {
            let mut meshes = self.combat.build_viewmodel_meshes(
                self.player.camera(),
                self.runtime_state.selected_weapon_item(),
            );
            meshes.extend(hud::build_hud_meshes(
                self.player.camera(),
                &self.runtime_state,
                &self.chat,
            ));
            meshes
        };

        let visible_chunk_coords = self.voxel_world.visible_chunk_coords(
            self.player.position(),
            CHUNK_VISIBILITY_RADIUS,
            camera_matrices.view,
            camera_matrices.projection,
        );

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        if self.world_meshes_dirty {
            self.cached_world_meshes = self.runtime_state.build_world_meshes();
            renderer.replace_static_model_meshes(self.cached_world_meshes.clone());
            self.world_meshes_dirty = false;
        }

        for update in chunk_updates {
            match update {
                ChunkMeshUpdate::Upsert { coord, mesh } => {
                    renderer.enqueue_chunk_mesh_upload(coord, mesh);
                }
                ChunkMeshUpdate::Remove { coord } => {
                    renderer.enqueue_chunk_mesh_remove(coord);
                }
            }
        }

        renderer.set_visible_chunks(visible_chunk_coords);
        renderer.replace_dynamic_model_meshes(dynamic_world_meshes);
        renderer.replace_viewmodel_meshes(viewmodel_meshes);
        renderer.update_camera_matrices(camera_matrices);

        if self.frame_index.is_multiple_of(240) {
            tracing::debug!(
                backend = renderer.name(),
                cam_x = self.player.position().x,
                cam_y = self.player.position().y,
                cam_z = self.player.position().z,
                cam_yaw = self.player.yaw(),
                cam_pitch = self.player.pitch(),
                "camera movement snapshot"
            );
        }

        if let Err(err) = renderer.render(clear) {
            tracing::error!(?err, "render loop failed, exiting");
            event_loop.exit();
            return;
        }

        let frame_stats = renderer.take_voxel_frame_stats();
        self.stream_uploaded_chunks_since_log += frame_stats.uploaded_chunks as u64;
        self.stream_uploaded_bytes_since_log += frame_stats.uploaded_bytes as u64;
        self.stream_drawn_chunks_since_log += frame_stats.drawn_chunks as u64;

        if self
            .frame_index
            .saturating_sub(self.last_stream_telemetry_frame)
            >= STREAM_TELEMETRY_INTERVAL_FRAMES
        {
            tracing::debug!(
                uploaded_chunks = self.stream_uploaded_chunks_since_log,
                uploaded_kib = self.stream_uploaded_bytes_since_log / 1024,
                drawn_chunks = self.stream_drawn_chunks_since_log,
                renderer_upload_backlog = frame_stats.pending_uploads,
                runtime_mesh_backlog,
                voxel_loaded = voxel_report.loaded_chunks,
                voxel_pending = voxel_report.pending_chunks,
                "voxel streaming telemetry"
            );

            self.stream_uploaded_chunks_since_log = 0;
            self.stream_uploaded_bytes_since_log = 0;
            self.stream_drawn_chunks_since_log = 0;
            self.last_stream_telemetry_frame = self.frame_index;
        }

        self.frame_index += 1;

        // soft frame pacing when vsync is disabled
        if !self.config.graphics.vsync {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn init_window_and_renderer(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.window.is_some() {
            return Ok(());
        }

        let attrs = WindowAttributes::default()
            .with_title(self.config.window.title.clone())
            .with_inner_size(PhysicalSize::new(
                self.config.window.width,
                self.config.window.height,
            ));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .context("failed to create main window")?,
        );

        let mut renderer = Renderer::new(
            window.clone(),
            self.config.graphics.backend,
            self.config.graphics.vsync,
        )
        .context("failed to initialize renderer")?;

        renderer.set_chunk_upload_budget_bytes_per_frame(CHUNK_UPLOAD_BUDGET_BYTES_PER_FRAME);

        tracing::info!(
            backend = renderer.name(),
            chunk_upload_budget_bytes = CHUNK_UPLOAD_BUDGET_BYTES_PER_FRAME,
            "renderer initialized"
        );

        self.renderer = Some(renderer);
        self.window = Some(window);
        self.set_mouse_captured(false);
        tracing::info!(
            "controls: menu with arrows/WASD + Enter, play with LMB/E hitscan and RMB/Q projectile, ESC opens menu"
        );
        Ok(())
    }
}

impl ApplicationHandler for EngineApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(err) = self.init_window_and_renderer(event_loop) {
            tracing::error!(
                ?err,
                "engine startup failed during window/renderer initialization; exiting"
            );
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(main_window_id) = self.window.as_ref().map(|window| window.id()) else {
            return;
        };

        if main_window_id != window_id {
            return;
        }

        let focus_lost = matches!(event, WindowEvent::Focused(false));

        self.input.handle_window_event(&event);
        if focus_lost {
            self.input.clear();
            self.set_mouse_captured(false);
        }

        if self.input.consume_key_press(KeyCode::Escape) {
            self.save_selected_world_runtime();
            self.set_mouse_captured(false);
        }

        if self.input.consume_key_press(KeyCode::F1) {
            self.set_mouse_captured(!self.mouse_captured);
        }
        if self.input.consume_key_press(KeyCode::Tab) {
            self.set_mouse_captured(!self.mouse_captured);
        }

        match event {
            WindowEvent::CloseRequested => {
                self.save_selected_world_runtime();
                event_loop.exit();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !self.mouse_captured {
                    self.set_mouse_captured(true);
                }

                if self.mouse_captured && !self.menu_open && !self.chat.is_open() {
                    self.primary_fire_requested = true;
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if self.mouse_captured
                    && !self.menu_open
                    && !self.chat.is_open()
                    && self.runtime_state.consume_throwable()
                {
                    let origin = self.player.position();
                    let direction = self.player.forward().normalize_or_zero();
                    self.combat.fire_weapon(
                        crate::inventory::ItemId::Grenade,
                        &mut self.runtime_state.enemies,
                        origin,
                        direction,
                        None,
                    );
                    self.chat.push_system_line("THREW GRENADE");
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(new_size);
                }

                if self.can_render()
                    && let Some(window) = self.window.as_ref()
                {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let (Some(renderer), Some(window)) =
                    (self.renderer.as_mut(), self.window.as_ref())
                {
                    renderer.resize(window.inner_size());
                }

                if self.can_render()
                    && let Some(window) = self.window.as_ref()
                {
                    window.request_redraw();
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.window_occluded = occluded;
                self.last_frame = Instant::now();

                if !occluded
                    && self.can_render()
                    && let Some(window) = self.window.as_ref()
                {
                    window.request_redraw();
                }
            }
            WindowEvent::Focused(true) => {
                if self.can_render()
                    && let Some(window) = self.window.as_ref()
                {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.render(event_loop);
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if self.mouse_captured {
            if let DeviceEvent::Button {
                button,
                state: ElementState::Pressed,
            } = &event
                && !self.menu_open
                && !self.chat.is_open()
            {
                if *button == 0 {
                    self.primary_fire_requested = true;
                }
            }
            self.input.handle_device_event(&event);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.can_render() {
            return;
        }

        if let (Some(window), Some(renderer)) = (self.window.as_ref(), self.renderer.as_ref()) {
            renderer.request_redraw(window);
        }
    }
}

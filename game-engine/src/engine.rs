use std::{
    io::ErrorKind,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use glam::{IVec3, Vec2, Vec3};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::KeyCode,
    window::{CursorGrabMode, Window, WindowAttributes, WindowId},
};

use crate::{
    config::AppConfig,
    ecs::{resources::Time, systems},
    input::handler::InputHandler,
    renderer::{
        CameraMatrices, Renderer,
        backend_trait::{Backend, ClearColor},
    },
    world::{
        camera::{Camera, CameraController},
        scene::Scene,
        scene_manager::SceneManager,
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
const PLAYER_EYE_TO_FEET: f32 = 1.6;
const PLAYER_COLLISION_RADIUS: f32 = 0.25;
const SPAWN_SURFACE_SEARCH_TOP_Y: i32 = 96;
const SPAWN_SURFACE_SEARCH_BOTTOM_Y: i32 = -32;
const SPAWN_EYE_CLEARANCE: f32 = 0.15;
const PLAYER_GRAVITY: f32 = 24.0;
const PLAYER_JUMP_SPEED: f32 = 8.0;
const PLAYER_MAX_FALL_SPEED: f32 = 30.0;
const PLAYER_GROUND_PROBE_EPSILON: f32 = 0.08;
const PLAYER_VERTICAL_SWEEP_STEP: f32 = 0.05;
const PLAYER_TORSO_PROBE_FROM_EYE: f32 = 0.8;
const PLAYER_FEET_CLEARANCE_PROBE: f32 = 0.05;

fn default_voxel_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|parallelism| parallelism.get().saturating_sub(1).max(1))
        .unwrap_or(1)
        .min(6)
}

struct EngineApp {
    config: AppConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    input: InputHandler,
    world: legion::World,
    scene_manager: SceneManager,
    voxel_world: VoxelWorld,
    camera: Camera,
    camera_controller: CameraController,
    start: Instant,
    last_frame: Instant,
    frame_index: u64,
    window_occluded: bool,
    stream_uploaded_chunks_since_log: u64,
    stream_uploaded_bytes_since_log: u64,
    stream_drawn_chunks_since_log: u64,
    last_stream_telemetry_frame: u64,
    spawn_aligned_to_world: bool,
    mouse_captured: bool,
    menu_open: bool,
    vertical_velocity: f32,
    is_grounded: bool,
}

impl EngineApp {
    fn new(config: AppConfig) -> Self {
        let mut scene_manager = SceneManager::new(Scene::demo());

        match scene_manager.load_active_scene() {
            Ok(path) => {
                tracing::info!(
                    scene = scene_manager.active_scene_id(),
                    entities = scene_manager.current_scene().entities.len(),
                    path = %path.display(),
                    "startup scene loaded"
                );
            }
            Err(err) => {
                if err
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io_err| io_err.kind() == ErrorKind::NotFound)
                {
                    tracing::info!(
                        scene = scene_manager.active_scene_id(),
                        "startup scene file missing, using built-in demo scene"
                    );
                } else {
                    tracing::warn!(
                        ?err,
                        scene = scene_manager.active_scene_id(),
                        "failed to load startup scene, using built-in demo scene"
                    );
                }
            }
        }

        let mut world = systems::bootstrap_world();
        if let Err(err) = scene_manager.apply_current_scene_to_world(&mut world) {
            tracing::warn!(
                ?err,
                scene = scene_manager.active_scene_id(),
                "failed to apply startup scene into ECS world, using bootstrap world"
            );
        }

        Self {
            camera_controller: CameraController {
                mouse_sensitivity: config.input.mouse_sensitivity,
                ..CameraController::default()
            },
            config,
            window: None,
            renderer: None,
            input: InputHandler::default(),
            world,
            scene_manager,
            voxel_world: VoxelWorld::new(0xC0FF_EE42, default_voxel_worker_count()),
            camera: Camera::default(),
            start: Instant::now(),
            last_frame: Instant::now(),
            frame_index: 0,
            window_occluded: false,
            stream_uploaded_chunks_since_log: 0,
            stream_uploaded_bytes_since_log: 0,
            stream_drawn_chunks_since_log: 0,
            last_stream_telemetry_frame: 0,
            spawn_aligned_to_world: false,
            mouse_captured: false,
            menu_open: false,
            vertical_velocity: 0.0,
            is_grounded: false,
        }
    }

    fn handle_scene_hotkeys(&mut self) {
        if self.input.consume_key_press(KeyCode::F5) {
            match self.scene_manager.save_world_to_active_scene(&self.world) {
                Ok(path) => {
                    tracing::info!(
                        scene = self.scene_manager.active_scene_id(),
                        entities = self.scene_manager.current_scene().entities.len(),
                        path = %path.display(),
                        "scene saved"
                    );
                }
                Err(err) => {
                    tracing::error!(
                        ?err,
                        scene = self.scene_manager.active_scene_id(),
                        "failed to save scene"
                    );
                }
            }
        }

        if self.input.consume_key_press(KeyCode::F9) {
            match self
                .scene_manager
                .load_active_scene_into_world(&mut self.world)
            {
                Ok(path) => {
                    tracing::info!(
                        scene = self.scene_manager.active_scene_id(),
                        entities = self.scene_manager.current_scene().entities.len(),
                        path = %path.display(),
                        "scene loaded"
                    );
                }
                Err(err) => {
                    tracing::error!(
                        ?err,
                        scene = self.scene_manager.active_scene_id(),
                        "failed to load scene"
                    );
                }
            }
        }

        if self.input.consume_key_press(KeyCode::F6) {
            match self
                .scene_manager
                .save_world_to_slot(&self.world, SceneManager::QUICK_SCENE_ID)
            {
                Ok(path) => {
                    tracing::info!(
                        slot = SceneManager::QUICK_SCENE_ID,
                        entities = self.scene_manager.current_scene().entities.len(),
                        path = %path.display(),
                        "quick scene saved"
                    );
                }
                Err(err) => {
                    tracing::error!(
                        ?err,
                        slot = SceneManager::QUICK_SCENE_ID,
                        "failed to save quick scene"
                    );
                }
            }
        }

        if self.input.consume_key_press(KeyCode::F10) {
            match self
                .scene_manager
                .load_scene_slot_into_world(&mut self.world, SceneManager::QUICK_SCENE_ID)
            {
                Ok(path) => {
                    tracing::info!(
                        slot = SceneManager::QUICK_SCENE_ID,
                        entities = self.scene_manager.current_scene().entities.len(),
                        path = %path.display(),
                        "quick scene loaded"
                    );
                }
                Err(err) => {
                    tracing::error!(
                        ?err,
                        slot = SceneManager::QUICK_SCENE_ID,
                        "failed to load quick scene"
                    );
                }
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

    fn try_align_spawn_to_surface(&mut self) {
        if self.spawn_aligned_to_world {
            return;
        }

        let column_x = self.camera.position.x.floor() as i32;
        let column_z = self.camera.position.z.floor() as i32;

        for y in (SPAWN_SURFACE_SEARCH_BOTTOM_Y..=SPAWN_SURFACE_SEARCH_TOP_Y).rev() {
            let world = IVec3::new(column_x, y, column_z);
            if self.voxel_world.block_at_world(world).is_some() {
                self.camera.position.y = y as f32 + 1.0 + PLAYER_EYE_TO_FEET + SPAWN_EYE_CLEARANCE;
                self.vertical_velocity = 0.0;
                self.is_grounded = true;
                self.spawn_aligned_to_world = true;
                tracing::info!(
                    cam_x = self.camera.position.x,
                    cam_y = self.camera.position.y,
                    cam_z = self.camera.position.z,
                    "camera spawn aligned above terrain"
                );
                return;
            }
        }
    }

    fn set_mouse_captured(&mut self, captured: bool) {
        if self.mouse_captured == captured {
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

    fn collision_offsets() -> [Vec3; 5] {
        [
            Vec3::ZERO,
            Vec3::new(PLAYER_COLLISION_RADIUS, 0.0, 0.0),
            Vec3::new(-PLAYER_COLLISION_RADIUS, 0.0, 0.0),
            Vec3::new(0.0, 0.0, PLAYER_COLLISION_RADIUS),
            Vec3::new(0.0, 0.0, -PLAYER_COLLISION_RADIUS),
        ]
    }

    fn is_camera_position_walkable(&self, position: Vec3) -> bool {
        for offset in Self::collision_offsets() {
            let eye_probe = position + offset;
            let torso_probe = eye_probe - Vec3::Y * PLAYER_TORSO_PROBE_FROM_EYE;
            let feet_clear_probe =
                eye_probe - Vec3::Y * (PLAYER_EYE_TO_FEET - PLAYER_FEET_CLEARANCE_PROBE);

            if self
                .voxel_world
                .block_at_world(eye_probe.floor().as_ivec3())
                .is_some()
                || self
                    .voxel_world
                    .block_at_world(torso_probe.floor().as_ivec3())
                    .is_some()
                || self
                    .voxel_world
                    .block_at_world(feet_clear_probe.floor().as_ivec3())
                    .is_some()
            {
                return false;
            }
        }

        true
    }

    fn is_standing_on_solid_ground(&self, position: Vec3) -> bool {
        for offset in Self::collision_offsets() {
            let ground_probe =
                position + offset - Vec3::Y * (PLAYER_EYE_TO_FEET + PLAYER_GROUND_PROBE_EPSILON);
            if self
                .voxel_world
                .block_at_world(ground_probe.floor().as_ivec3())
                .is_some()
            {
                return true;
            }
        }

        false
    }

    fn move_camera_vertically_with_collision(&mut self, delta_y: f32) {
        if delta_y.abs() <= f32::EPSILON {
            return;
        }

        let direction = delta_y.signum();
        let mut remaining = delta_y;

        while remaining.abs() > f32::EPSILON {
            let step = if remaining.abs() > PLAYER_VERTICAL_SWEEP_STEP {
                direction * PLAYER_VERTICAL_SWEEP_STEP
            } else {
                remaining
            };

            let candidate = self.camera.position + Vec3::new(0.0, step, 0.0);
            if self.is_camera_position_walkable(candidate) {
                self.camera.position = candidate;
                remaining -= step;
            } else {
                if step < 0.0 {
                    self.is_grounded = true;
                }
                self.vertical_velocity = 0.0;
                break;
            }
        }
    }

    fn apply_jump_and_gravity(&mut self, dt_seconds: f32) {
        if !self.spawn_aligned_to_world || self.menu_open {
            self.vertical_velocity = 0.0;
            return;
        }

        self.is_grounded = self.is_standing_on_solid_ground(self.camera.position);

        if self.input.consume_key_press(KeyCode::Space) && self.is_grounded {
            self.vertical_velocity = PLAYER_JUMP_SPEED;
            self.is_grounded = false;
        }

        if !self.is_grounded || self.vertical_velocity > 0.0 {
            self.vertical_velocity =
                (self.vertical_velocity - PLAYER_GRAVITY * dt_seconds).max(-PLAYER_MAX_FALL_SPEED);
            self.move_camera_vertically_with_collision(self.vertical_velocity * dt_seconds);
        }

        if self.is_standing_on_solid_ground(self.camera.position) && self.vertical_velocity <= 0.0 {
            self.is_grounded = true;
            self.vertical_velocity = 0.0;
        }
    }

    fn apply_block_collision_to_camera_movement(&mut self, previous_position: Vec3) {
        if !self.spawn_aligned_to_world {
            return;
        }

        let desired = self.camera.position;
        self.camera.position = previous_position;

        let x_candidate = Vec3::new(desired.x, self.camera.position.y, self.camera.position.z);
        if self.is_camera_position_walkable(x_candidate) {
            self.camera.position.x = x_candidate.x;
        }

        let z_candidate = Vec3::new(self.camera.position.x, self.camera.position.y, desired.z);
        if self.is_camera_position_walkable(z_candidate) {
            self.camera.position.z = z_candidate.z;
        }

        if !self.is_camera_position_walkable(self.camera.position) {
            self.camera.position = previous_position;
        }
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

        let t = self.start.elapsed().as_secs_f32();
        let mut clear = ClearColor::BLACK;
        clear.r = (0.5 + 0.5 * (t * 0.7).sin()) as f64;
        clear.g = (0.5 + 0.5 * (t * 1.3).cos()) as f64;
        clear.b = (0.5 + 0.5 * (t * 0.9).sin()) as f64;

        let dt_seconds = dt.as_secs_f32();

        systems::tick(
            &mut self.world,
            Time {
                delta_seconds: dt_seconds,
                frame_index: self.frame_index,
            },
        );

        let move_axis = if self.menu_open {
            Vec3::ZERO
        } else {
            self.input.frame_movement_axis()
        };
        let mouse_delta = if self.mouse_captured {
            self.input.take_mouse_delta()
        } else {
            let _ = self.input.take_mouse_delta();
            Vec2::ZERO
        };
        let previous_camera_position = self.camera.position;
        self.camera_controller
            .update(&mut self.camera, move_axis, mouse_delta, dt_seconds);
        self.apply_block_collision_to_camera_movement(previous_camera_position);

        let voxel_report = self.voxel_world.tick(self.camera.position);
        self.try_align_spawn_to_surface();
        self.apply_jump_and_gravity(dt_seconds);
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

        if self.mouse_captured && self.input.consume_key_press(KeyCode::KeyE) {
            let hit = self
                .voxel_world
                .raycast(self.camera.position, self.camera.forward(), 8.0);

            match hit {
                Some(hit) => {
                    tracing::info!(
                        block = ?hit.block,
                        block_x = hit.block_pos.x,
                        block_y = hit.block_pos.y,
                        block_z = hit.block_pos.z,
                        distance = hit.distance,
                        "voxel pick hit"
                    );
                }
                None => {
                    tracing::info!("voxel pick miss");
                }
            }
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
            view: self.camera.view_matrix(),
            projection: self.camera.projection_matrix(aspect_ratio),
        };

        let visible_chunk_coords = self.voxel_world.visible_chunk_coords(
            self.camera.position,
            CHUNK_VISIBILITY_RADIUS,
            camera_matrices.view,
            camera_matrices.projection,
        );

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

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
        renderer.update_camera_matrices(camera_matrices);

        if self.frame_index.is_multiple_of(240) {
            tracing::debug!(
                backend = renderer.name(),
                cam_x = self.camera.position.x,
                cam_y = self.camera.position.y,
                cam_z = self.camera.position.z,
                cam_yaw = self.camera.yaw,
                cam_pitch = self.camera.pitch,
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
        self.set_mouse_captured(true);
        tracing::info!("controls: WASD move, SPACE jump, ESC menu/unlock, left-click recapture, TAB/F1 toggle capture");
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
            self.set_mouse_captured(false);
        }

        if self.input.consume_key_press(KeyCode::F1) {
            self.set_mouse_captured(!self.mouse_captured);
        }
        if self.input.consume_key_press(KeyCode::Tab) {
            self.set_mouse_captured(!self.mouse_captured);
        }

        self.handle_scene_hotkeys();

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !self.mouse_captured {
                    self.set_mouse_captured(true);
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

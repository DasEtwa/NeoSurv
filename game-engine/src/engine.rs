use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use glam::{IVec3, Quat, Vec3};
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
    game::model::{self, StaticModelSpawn},
    input::handler::InputHandler,
    player::Player,
    renderer::{
        CameraMatrices, Renderer, StaticModelMesh,
        backend_trait::{Backend, ClearColor},
    },
    world::voxel::{ChunkMeshUpdate, VoxelWorld, block::BlockType},
};
use crate::world::camera::Camera;

#[derive(Debug, Clone, Copy)]
struct PendingBlockWrite {
    world: IVec3,
    block: BlockType,
}

#[derive(Debug, Clone)]
struct CreatureTarget {
    base_block: IVec3,
    hp: i32,
}

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
const WEAPON_MODEL_REL_PATH: &str = "assets/models/pistol_1/Pistol_1.obj";
const BORDER_WALL_HEIGHT: f32 = 48.0;
const BORDER_WALL_Y_MIN: f32 = -4.0;
const BORDER_WALL_THICKNESS: f32 = 2.0;
const VIEWMODEL_DISTANCE: f32 = 0.36;
const VIEWMODEL_RIGHT_OFFSET: f32 = 0.12;
const VIEWMODEL_DOWN_OFFSET: f32 = 0.18;
const VIEWMODEL_SCALE: f32 = 0.52;
const VIEWMODEL_RECOIL_DISTANCE: f32 = 0.06;
const VIEWMODEL_MUZZLE_FLASH_TIME: f32 = 0.07;
const VIEWMODEL_RECOIL_TIME: f32 = 0.10;
const WEAPON_HITSCAN_RANGE: f32 = 96.0;
const WEAPON_SHOT_DAMAGE: i32 = 34;
const DUMMY_MAX_HP: i32 = 100;
const MAX_PENDING_BLOCK_WRITES_PER_FRAME: usize = 4096;
const DUMMY_HEIGHT_OVER_SURFACE: i32 = 1;
const BOT_VISIBLE_COUNT: usize = 5;

fn default_voxel_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|parallelism| parallelism.get().saturating_sub(1).max(1))
        .unwrap_or(1)
        .min(6)
}

fn project_asset_path(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path)
}

fn load_weapon_source_meshes() -> Vec<StaticModelMesh> {
    let spawn = StaticModelSpawn {
        position: Vec3::ZERO,
        uniform_scale: 1.0,
    };
    let weapon_path = project_asset_path(WEAPON_MODEL_REL_PATH);

    match model::load_static_obj(&weapon_path, spawn) {
        Ok(loaded_model) => {
            tracing::info!(
                path = %loaded_model.source_path.display(),
                mesh_count = loaded_model.meshes.len(),
                material_libraries = loaded_model.material_libraries.len(),
                referenced_diffuse_textures = loaded_model.referenced_diffuse_textures.len(),
                "static OBJ model loaded"
            );

            if !loaded_model.referenced_diffuse_textures.is_empty() {
                tracing::info!(
                    ?loaded_model.referenced_diffuse_textures,
                    "OBJ diffuse textures were detected but are not rendered yet"
                );
            }

            normalize_static_model_meshes(
                loaded_model.meshes.into_iter().map(Into::into).collect(),
                1.0,
            )
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                path = %weapon_path.display(),
                "failed to load weapon OBJ model"
            );
            Vec::new()
        }
    }
}

fn normalize_static_model_meshes(
    meshes: Vec<StaticModelMesh>,
    target_max_extent: f32,
) -> Vec<StaticModelMesh> {
    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);

    for mesh in &meshes {
        for vertex in &mesh.vertices {
            let pos = Vec3::from_array(vertex.position);
            bounds_min = bounds_min.min(pos);
            bounds_max = bounds_max.max(pos);
        }
    }

    if !bounds_min.is_finite() || !bounds_max.is_finite() {
        return meshes;
    }

    let center = (bounds_min + bounds_max) * 0.5;
    let extent = (bounds_max - bounds_min).max(Vec3::splat(0.0001));
    let scale = target_max_extent / extent.max_element().max(0.0001);

    meshes
        .into_iter()
        .map(|mesh| StaticModelMesh {
            label: mesh.label,
            vertices: mesh
                .vertices
                .into_iter()
                .map(|vertex| {
                    let pos = (Vec3::from_array(vertex.position) - center) * scale;
                    let mut out = vertex;
                    out.position = pos.to_array();
                    out
                })
                .collect(),
            indices: mesh.indices,
        })
        .collect()
}

fn make_box_mesh(label: impl Into<String>, min: Vec3, max: Vec3, color: [f32; 4]) -> StaticModelMesh {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];

    let faces = [
        ([0usize, 1, 2, 3], [0.0, 0.0, -1.0]),
        ([5usize, 4, 7, 6], [0.0, 0.0, 1.0]),
        ([4usize, 0, 3, 7], [-1.0, 0.0, 0.0]),
        ([1usize, 5, 6, 2], [1.0, 0.0, 0.0]),
        ([3usize, 2, 6, 7], [0.0, 1.0, 0.0]),
        ([4usize, 5, 1, 0], [0.0, -1.0, 0.0]),
    ];

    for (face_index, (quad, normal)) in faces.into_iter().enumerate() {
        let base = (face_index * 4) as u32;
        let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

        for (corner_index, uv) in quad.into_iter().zip(uv) {
            vertices.push(crate::renderer::StaticModelVertex {
                position: corners[corner_index].to_array(),
                normal,
                uv,
                color,
            });
        }

        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    StaticModelMesh {
        label: label.into(),
        vertices,
        indices,
    }
}

fn transform_static_mesh(
    mesh: &StaticModelMesh,
    label: impl Into<String>,
    rotation: Quat,
    translation: Vec3,
    scale: f32,
) -> StaticModelMesh {
    StaticModelMesh {
        label: label.into(),
        vertices: mesh
            .vertices
            .iter()
            .copied()
            .map(|vertex| {
                let position = rotation * (Vec3::from_array(vertex.position) * scale) + translation;
                let normal = (rotation * Vec3::from_array(vertex.normal)).normalize_or_zero();
                crate::renderer::StaticModelVertex {
                    position: position.to_array(),
                    normal: normal.to_array(),
                    uv: vertex.uv,
                    color: vertex.color,
                }
            })
            .collect(),
        indices: mesh.indices.clone(),
    }
}

fn transform_viewmodel_mesh(
    mesh: &StaticModelMesh,
    label: impl Into<String>,
    camera: &Camera,
    local_rotation: Quat,
    local_offset: Vec3,
    scale: f32,
) -> StaticModelMesh {
    let forward = camera.forward().normalize_or_zero();
    let right = camera.right().normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let origin = camera.position
        + right * local_offset.x
        + up * local_offset.y
        + forward * local_offset.z;

    StaticModelMesh {
        label: label.into(),
        vertices: mesh
            .vertices
            .iter()
            .copied()
            .map(|vertex| {
                let local_position = local_rotation * (Vec3::from_array(vertex.position) * scale);
                let local_normal =
                    (local_rotation * Vec3::from_array(vertex.normal)).normalize_or_zero();

                let world_position = origin
                    + right * local_position.x
                    + up * local_position.y
                    + forward * local_position.z;
                let world_normal = (right * local_normal.x
                    + up * local_normal.y
                    + forward * local_normal.z)
                    .normalize_or_zero();

                crate::renderer::StaticModelVertex {
                    position: world_position.to_array(),
                    normal: world_normal.to_array(),
                    uv: vertex.uv,
                    color: vertex.color,
                }
            })
            .collect(),
        indices: mesh.indices.clone(),
    }
}

fn ray_aabb_distance(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv_dir = Vec3::new(
        if direction.x.abs() > f32::EPSILON {
            1.0 / direction.x
        } else {
            f32::INFINITY
        },
        if direction.y.abs() > f32::EPSILON {
            1.0 / direction.y
        } else {
            f32::INFINITY
        },
        if direction.z.abs() > f32::EPSILON {
            1.0 / direction.z
        } else {
            f32::INFINITY
        },
    );

    let t1 = (min - origin) * inv_dir;
    let t2 = (max - origin) * inv_dir;
    let t_min = t1.min(t2);
    let t_max = t1.max(t2);

    let near = t_min.max_element();
    let far = t_max.min_element();

    if far < 0.0 || near > far {
        None
    } else {
        Some(near.max(0.0))
    }
}

struct EngineApp {
    config: AppConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
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
    shoot_requested: bool,
    shot_flash_timer: f32,
    shot_recoil_timer: f32,
    dummies_spawned: bool,
    pending_block_writes: VecDeque<PendingBlockWrite>,
    creatures: Vec<CreatureTarget>,
    dummy_spawned_slots: HashSet<usize>,
    dummy_spawn_anchor: Option<IVec3>,
    dummy_kills: u32,
    weapon_source_meshes: Vec<StaticModelMesh>,
}

impl EngineApp {
    fn new(config: AppConfig) -> Self {
        let mut app = Self {
            player: Player::new(config.input.mouse_sensitivity),
            config,
            window: None,
            renderer: None,
            input: InputHandler::default(),
            voxel_world: VoxelWorld::new(0xC0FF_EE42, default_voxel_worker_count()),
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
            shoot_requested: false,
            shot_flash_timer: 0.0,
            shot_recoil_timer: 0.0,
            dummies_spawned: false,
            pending_block_writes: VecDeque::new(),
            creatures: Vec::new(),
            dummy_spawned_slots: HashSet::new(),
            dummy_spawn_anchor: None,
            dummy_kills: 0,
            weapon_source_meshes: load_weapon_source_meshes(),
        };
        app.reset_run_state();
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
        self.menu_open = false;
        self.shoot_requested = false;
        self.shot_flash_timer = 0.0;
        self.shot_recoil_timer = 0.0;
        self.dummies_spawned = false;
        self.pending_block_writes.clear();
        self.creatures.clear();
        self.dummy_spawned_slots.clear();
        self.dummy_spawn_anchor = None;
        self.dummy_kills = 0;
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

    fn enqueue_block_write(&mut self, world: IVec3, block: BlockType) {
        self.pending_block_writes
            .push_back(PendingBlockWrite { world, block });
    }

    fn process_pending_block_writes(&mut self) {
        let budget = MAX_PENDING_BLOCK_WRITES_PER_FRAME.min(self.pending_block_writes.len());

        for _ in 0..budget {
            let Some(write) = self.pending_block_writes.pop_front() else {
                break;
            };

            if !self.voxel_world.set_block_world(write.world, write.block) {
                self.pending_block_writes.push_back(write);
            }
        }
    }

    fn find_surface_height(&self, x: i32, z: i32) -> Option<i32> {
        for y in (SPAWN_SURFACE_SEARCH_BOTTOM_Y..=SPAWN_SURFACE_SEARCH_TOP_Y).rev() {
            if self
                .voxel_world
                .block_at_world(IVec3::new(x, y, z))
                .is_some()
            {
                return Some(y);
            }
        }

        None
    }

    fn ensure_dummies_spawned(&mut self) {
        if self.dummies_spawned {
            return;
        }

        let planar_forward = Vec3::new(self.player.forward().x, 0.0, self.player.forward().z)
            .normalize_or_zero();
        let right = self.player.right().normalize_or_zero();
        let player_feet = self.player.position() - Vec3::Y * self.player.current_eye_to_feet();
        let spawn_origin = self
            .dummy_spawn_anchor
            .map(|anchor| Vec3::new(anchor.x as f32 + 0.5, player_feet.y, anchor.z as f32 + 0.5))
            .unwrap_or(player_feet);

        const BOT_LAYOUT: [(f32, f32); BOT_VISIBLE_COUNT] = [
            (-3.0, 11.0),
            (0.0, 13.0),
            (3.0, 15.0),
            (-5.0, 19.0),
            (5.0, 21.0),
        ];

        self.creatures.clear();
        self.dummy_spawned_slots.clear();

        for (slot, (side_offset, forward_offset)) in BOT_LAYOUT.into_iter().enumerate() {
            let target = spawn_origin + right * side_offset + planar_forward * forward_offset;
            let x = target.x.round() as i32;
            let z = target.z.round() as i32;
            let surface = self
                .find_surface_height(x, z)
                .unwrap_or_else(|| player_feet.y.floor() as i32 - 1);
            let base = IVec3::new(x, surface + DUMMY_HEIGHT_OVER_SURFACE, z);

            self.creatures.push(CreatureTarget {
                base_block: base,
                hp: DUMMY_MAX_HP,
            });
            self.dummy_spawned_slots.insert(slot);
            tracing::info!(slot, x, y = base.y, z, "bot spawned");
        }

        self.dummies_spawned = true;
        tracing::info!(dummy_count = self.creatures.len(), "bots ready");
    }

    fn fire_hitscan_shot(&mut self) {
        let origin = self.player.position();
        let direction = self.player.forward().normalize_or_zero();
        let mut best_hit: Option<(usize, f32)> = None;
        self.shot_flash_timer = VIEWMODEL_MUZZLE_FLASH_TIME;
        self.shot_recoil_timer = VIEWMODEL_RECOIL_TIME;

        for (index, creature) in self.creatures.iter().enumerate() {
            let min = Vec3::new(
                creature.base_block.x as f32 - 0.45,
                creature.base_block.y as f32,
                creature.base_block.z as f32 - 0.45,
            );
            let max = Vec3::new(
                creature.base_block.x as f32 + 0.45,
                creature.base_block.y as f32 + 2.0,
                creature.base_block.z as f32 + 0.45,
            );

            let Some(distance) = ray_aabb_distance(origin, direction, min, max) else {
                continue;
            };

            if distance > WEAPON_HITSCAN_RANGE {
                continue;
            }

            match best_hit {
                Some((_, current_best)) if current_best <= distance => {}
                _ => best_hit = Some((index, distance)),
            }
        }

        let Some((hit_index, _)) = best_hit else {
            tracing::debug!("shot miss");
            return;
        };

        let remaining_hp = self.creatures[hit_index].hp - WEAPON_SHOT_DAMAGE;

        if remaining_hp <= 0 {
            let base = self.creatures[hit_index].base_block;
            self.creatures.swap_remove(hit_index);
            self.dummy_kills = self.dummy_kills.saturating_add(1);
            tracing::info!(
                x = base.x,
                y = base.y,
                z = base.z,
                kills = self.dummy_kills,
                remaining_dummies = self.creatures.len(),
                "2-block creature destroyed"
            );
        } else {
            let base = self.creatures[hit_index].base_block;
            self.creatures[hit_index].hp = remaining_hp;
            tracing::info!(
                x = base.x,
                y = base.y,
                z = base.z,
                hp = remaining_hp,
                "2-block creature hit"
            );
        }
    }

    fn build_border_meshes(&self) -> Vec<StaticModelMesh> {
        let half = WORLD_BORDER_HALF_EXTENT;
        let outer = half + BORDER_WALL_THICKNESS;
        let y_min = BORDER_WALL_Y_MIN;
        let y_max = BORDER_WALL_Y_MIN + BORDER_WALL_HEIGHT;
        let wall_color = [0.10, 0.16, 0.45, 1.0];

        vec![
            make_box_mesh(
                "border-north",
                Vec3::new(-outer, y_min, -outer),
                Vec3::new(outer, y_max, -half),
                wall_color,
            ),
            make_box_mesh(
                "border-south",
                Vec3::new(-outer, y_min, half),
                Vec3::new(outer, y_max, outer),
                wall_color,
            ),
            make_box_mesh(
                "border-west",
                Vec3::new(-outer, y_min, -half),
                Vec3::new(-half, y_max, half),
                wall_color,
            ),
            make_box_mesh(
                "border-east",
                Vec3::new(half, y_min, -half),
                Vec3::new(outer, y_max, half),
                wall_color,
            ),
        ]
    }

    fn build_creature_meshes(&self) -> Vec<StaticModelMesh> {
        let mut meshes = Vec::with_capacity(self.creatures.len() * 2);

        for (index, creature) in self.creatures.iter().enumerate() {
            let base = Vec3::new(
                creature.base_block.x as f32,
                creature.base_block.y as f32,
                creature.base_block.z as f32,
            );
            let hp_ratio = (creature.hp as f32 / DUMMY_MAX_HP as f32).clamp(0.0, 1.0);
            let body_color = [0.95, 0.20 + 0.45 * hp_ratio, 0.24, 1.0];
            let head_color = [1.0, 0.92, 0.78, 1.0];

            meshes.push(make_box_mesh(
                format!("creature-body-{index}"),
                base + Vec3::new(-0.45, 0.0, -0.35),
                base + Vec3::new(0.45, 1.3, 0.35),
                body_color,
            ));
            meshes.push(make_box_mesh(
                format!("creature-head-{index}"),
                base + Vec3::new(-0.34, 1.3, -0.34),
                base + Vec3::new(0.34, 2.0, 0.34),
                head_color,
            ));
        }

        meshes
    }

    fn build_weapon_meshes(&self) -> Vec<StaticModelMesh> {
        let mut meshes = Vec::new();
        let forward = self.player.forward().normalize_or_zero();
        let right = self.player.right().normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let recoil_ratio = (self.shot_recoil_timer / VIEWMODEL_RECOIL_TIME).clamp(0.0, 1.0);
        let weapon_offset = Vec3::new(
            VIEWMODEL_RIGHT_OFFSET,
            -VIEWMODEL_DOWN_OFFSET,
            VIEWMODEL_DISTANCE - recoil_ratio * VIEWMODEL_RECOIL_DISTANCE,
        );
        let model_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_z(-0.10)
            * Quat::from_rotation_x(0.05);

        if !self.weapon_source_meshes.is_empty() {
            meshes.extend(self.weapon_source_meshes.iter().enumerate().map(|(index, mesh)| {
                transform_viewmodel_mesh(
                    mesh,
                    format!("viewmodel-pistol-{index}"),
                    self.player.camera(),
                    model_rotation,
                    weapon_offset,
                    VIEWMODEL_SCALE,
                )
            }));
        } else {
            let weapon_origin = self.player.position()
                + forward * weapon_offset.z
                + right * weapon_offset.x
                + up * weapon_offset.y;

            meshes.push(make_box_mesh(
                "fallback-sidearm-slide",
                weapon_origin + right * -0.11 + up * -0.05 + forward * -0.20,
                weapon_origin + right * 0.11 + up * 0.05 + forward * 0.16,
                [0.08, 0.08, 0.10, 1.0],
            ));
            meshes.push(make_box_mesh(
                "fallback-sidearm-grip",
                weapon_origin + right * -0.05 + up * -0.22 + forward * -0.02,
                weapon_origin + right * 0.05 + up * -0.02 + forward * 0.09,
                [0.22, 0.12, 0.08, 1.0],
            ));
            meshes.push(make_box_mesh(
                "fallback-sidearm-barrel",
                weapon_origin + right * -0.03 + up * -0.01 + forward * 0.16,
                weapon_origin + right * 0.03 + up * 0.03 + forward * 0.28,
                [0.20, 0.20, 0.22, 1.0],
            ));
        }

        if self.shot_flash_timer > 0.0 {
            let flash_origin = self.player.position()
                + forward * (weapon_offset.z + 0.28)
                + right * (weapon_offset.x + 0.02)
                + up * (weapon_offset.y + 0.02);
            meshes.push(make_box_mesh(
                "muzzle-flash",
                flash_origin - Vec3::splat(0.08),
                flash_origin + Vec3::splat(0.08),
                [1.0, 0.92, 0.55, 1.0],
            ));
        }

        meshes
    }

    fn build_world_static_meshes(&self) -> Vec<StaticModelMesh> {
        self.build_creature_meshes()
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
        self.shot_flash_timer = (self.shot_flash_timer - dt_seconds).max(0.0);
        self.shot_recoil_timer = (self.shot_recoil_timer - dt_seconds).max(0.0);

        let voxel_world = &self.voxel_world;
        self.player.update_look_and_move(
            &mut self.input,
            self.mouse_captured,
            self.menu_open,
            dt_seconds,
            |world| voxel_world.block_at_world(world).is_some(),
        );
        self.clamp_to_world_border();

        let voxel_report = self.voxel_world.tick(self.player.position());
        if let Some(anchor) = self.player.try_align_spawn_to_surface(|world| {
            self.voxel_world.block_at_world(world).is_some()
        }) {
            self.dummy_spawn_anchor = Some(anchor);
        }
        self.player
            .apply_jump_and_gravity(&mut self.input, self.menu_open, dt_seconds, |world| {
                self.voxel_world.block_at_world(world).is_some()
            });
        self.clamp_to_world_border();
        self.ensure_dummies_spawned();
        self.process_pending_block_writes();

        if self.shoot_requested && self.mouse_captured && !self.menu_open {
            self.fire_hitscan_shot();
        }
        self.shoot_requested = false;

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
            self.fire_hitscan_shot();
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
        let world_static_meshes = self.build_world_static_meshes();
        let viewmodel_meshes = self.build_weapon_meshes();

        let visible_chunk_coords = self.voxel_world.visible_chunk_coords(
            self.player.position(),
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
        renderer.replace_static_model_meshes(world_static_meshes);
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
        self.set_mouse_captured(true);
        tracing::info!(
            "controls: WASD move, SHIFT sprint, V crouch, SPACE jump, LMB/E shoot (hitscan), ESC menu/unlock, left-click recapture, TAB/F1 toggle capture"
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
            self.set_mouse_captured(false);
        }

        if self.input.consume_key_press(KeyCode::F1) {
            self.set_mouse_captured(!self.mouse_captured);
        }
        if self.input.consume_key_press(KeyCode::Tab) {
            self.set_mouse_captured(!self.mouse_captured);
        }

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

                if self.mouse_captured && !self.menu_open {
                    self.shoot_requested = true;
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
                && (*button == 0 || *button == 1)
                && !self.menu_open
            {
                self.shoot_requested = true;
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

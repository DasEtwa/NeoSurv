use glam::{IVec3, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

use crate::{
    input::handler::InputHandler,
    world::camera::{Camera, CameraController},
};

const PLAYER_EYE_TO_FEET_STANDING: f32 = 1.6;
const PLAYER_EYE_TO_FEET_CROUCHING: f32 = 1.1;
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
const PLAYER_SPRINT_MULTIPLIER: f32 = 1.6;
const PLAYER_CROUCH_SPEED_MULTIPLIER: f32 = 0.45;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct SavedPlayerPose {
    pub(crate) position: [f32; 3],
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct Player {
    camera: Camera,
    camera_controller: CameraController,
    spawn_aligned_to_world: bool,
    vertical_velocity: f32,
    is_grounded: bool,
    is_crouching: bool,
}

impl Player {
    pub(crate) fn new(mouse_sensitivity: f32) -> Self {
        Self {
            camera_controller: CameraController {
                mouse_sensitivity,
                ..CameraController::default()
            },
            camera: Camera::default(),
            spawn_aligned_to_world: false,
            vertical_velocity: 0.0,
            is_grounded: false,
            is_crouching: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.camera = Camera::default();
        self.spawn_aligned_to_world = false;
        self.vertical_velocity = 0.0;
        self.is_grounded = false;
        self.is_crouching = false;
    }

    pub(crate) fn camera(&self) -> &Camera {
        &self.camera
    }

    pub(crate) fn position(&self) -> Vec3 {
        self.camera.position
    }

    pub(crate) fn yaw(&self) -> f32 {
        self.camera.yaw
    }

    pub(crate) fn pitch(&self) -> f32 {
        self.camera.pitch
    }

    pub(crate) fn forward(&self) -> Vec3 {
        self.camera.forward()
    }

    pub(crate) fn view_matrix(&self) -> glam::Mat4 {
        self.camera.view_matrix()
    }

    pub(crate) fn projection_matrix(&self, aspect_ratio: f32) -> glam::Mat4 {
        self.camera.projection_matrix(aspect_ratio)
    }

    pub(crate) fn current_eye_to_feet(&self) -> f32 {
        if self.is_crouching {
            PLAYER_EYE_TO_FEET_CROUCHING
        } else {
            PLAYER_EYE_TO_FEET_STANDING
        }
    }

    pub(crate) fn saved_pose(&self) -> SavedPlayerPose {
        SavedPlayerPose {
            position: self.camera.position.to_array(),
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
        }
    }

    pub(crate) fn restore_saved_pose(&mut self, pose: SavedPlayerPose) {
        self.camera.position = Vec3::from_array(pose.position);
        self.camera.yaw = pose.yaw;
        self.camera.pitch = pose.pitch;
        self.spawn_aligned_to_world = true;
        self.vertical_velocity = 0.0;
        self.is_grounded = false;
        self.is_crouching = false;
    }

    pub(crate) fn update_look_and_move<F>(
        &mut self,
        input: &mut InputHandler,
        mouse_captured: bool,
        menu_open: bool,
        dt_seconds: f32,
        mut is_solid: F,
    ) where
        F: FnMut(IVec3) -> bool,
    {
        let move_axis = if menu_open {
            Vec3::ZERO
        } else {
            input.frame_movement_axis()
        };

        let crouch_requested = !menu_open && input.is_key_pressed(KeyCode::KeyV);
        self.update_crouch_state(crouch_requested, &mut is_solid);

        let sprint_pressed = !menu_open
            && (input.is_key_pressed(KeyCode::ShiftLeft)
                || input.is_key_pressed(KeyCode::ShiftRight));
        let speed_multiplier = if self.is_crouching {
            PLAYER_CROUCH_SPEED_MULTIPLIER
        } else if sprint_pressed {
            PLAYER_SPRINT_MULTIPLIER
        } else {
            1.0
        };

        let mouse_delta = if mouse_captured {
            input.take_mouse_delta()
        } else {
            let _ = input.take_mouse_delta();
            Vec2::ZERO
        };

        let previous_camera_position = self.camera.position;
        self.camera_controller.update(
            &mut self.camera,
            move_axis,
            mouse_delta,
            dt_seconds,
            speed_multiplier,
        );
        self.apply_block_collision_to_camera_movement(previous_camera_position, &mut is_solid);
    }

    pub(crate) fn try_align_spawn_to_surface<F>(&mut self, mut is_solid: F) -> Option<IVec3>
    where
        F: FnMut(IVec3) -> bool,
    {
        if self.spawn_aligned_to_world {
            return None;
        }

        let column_x = self.camera.position.x.floor() as i32;
        let column_z = self.camera.position.z.floor() as i32;

        for y in (SPAWN_SURFACE_SEARCH_BOTTOM_Y..=SPAWN_SURFACE_SEARCH_TOP_Y).rev() {
            let world = IVec3::new(column_x, y, column_z);
            if is_solid(world) {
                self.camera.position.y =
                    y as f32 + 1.0 + self.current_eye_to_feet() + SPAWN_EYE_CLEARANCE;
                self.vertical_velocity = 0.0;
                self.is_grounded = true;
                self.spawn_aligned_to_world = true;
                tracing::info!(
                    cam_x = self.camera.position.x,
                    cam_y = self.camera.position.y,
                    cam_z = self.camera.position.z,
                    "camera spawn aligned above terrain"
                );
                return Some(IVec3::new(column_x, 0, column_z));
            }
        }

        None
    }

    pub(crate) fn apply_jump_and_gravity<F>(
        &mut self,
        input: &mut InputHandler,
        menu_open: bool,
        dt_seconds: f32,
        mut is_solid: F,
    ) where
        F: FnMut(IVec3) -> bool,
    {
        if !self.spawn_aligned_to_world || menu_open {
            self.vertical_velocity = 0.0;
            return;
        }

        self.is_grounded = self.is_standing_on_solid_ground(self.camera.position, &mut is_solid);

        if input.consume_key_press(KeyCode::Space) && self.is_grounded {
            self.vertical_velocity = PLAYER_JUMP_SPEED;
            self.is_grounded = false;
        }

        if !self.is_grounded || self.vertical_velocity > 0.0 {
            self.vertical_velocity =
                (self.vertical_velocity - PLAYER_GRAVITY * dt_seconds).max(-PLAYER_MAX_FALL_SPEED);
            self.move_camera_vertically_with_collision(
                self.vertical_velocity * dt_seconds,
                &mut is_solid,
            );
        }

        if self.is_standing_on_solid_ground(self.camera.position, &mut is_solid)
            && self.vertical_velocity <= 0.0
        {
            self.is_grounded = true;
            self.vertical_velocity = 0.0;
        }
    }

    pub(crate) fn clamp_to_world_border(&mut self, walkable_half_extent: f32) {
        self.camera.position.x = self
            .camera
            .position
            .x
            .clamp(-walkable_half_extent, walkable_half_extent);
        self.camera.position.z = self
            .camera
            .position
            .z
            .clamp(-walkable_half_extent, walkable_half_extent);
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

    fn is_camera_position_walkable_with_eye_to_feet<F>(
        &self,
        position: Vec3,
        eye_to_feet: f32,
        is_solid: &mut F,
    ) -> bool
    where
        F: FnMut(IVec3) -> bool,
    {
        for offset in Self::collision_offsets() {
            let eye_probe = position + offset;
            let torso_probe = eye_probe - Vec3::Y * PLAYER_TORSO_PROBE_FROM_EYE;
            let feet_clear_probe =
                eye_probe - Vec3::Y * (eye_to_feet - PLAYER_FEET_CLEARANCE_PROBE);

            if is_solid(eye_probe.floor().as_ivec3())
                || is_solid(torso_probe.floor().as_ivec3())
                || is_solid(feet_clear_probe.floor().as_ivec3())
            {
                return false;
            }
        }

        true
    }

    fn is_camera_position_walkable<F>(&self, position: Vec3, is_solid: &mut F) -> bool
    where
        F: FnMut(IVec3) -> bool,
    {
        self.is_camera_position_walkable_with_eye_to_feet(
            position,
            self.current_eye_to_feet(),
            is_solid,
        )
    }

    fn update_crouch_state<F>(&mut self, crouch_requested: bool, is_solid: &mut F)
    where
        F: FnMut(IVec3) -> bool,
    {
        if crouch_requested {
            if self.is_crouching {
                return;
            }

            let delta = PLAYER_EYE_TO_FEET_STANDING - PLAYER_EYE_TO_FEET_CROUCHING;
            self.camera.position.y -= delta;
            self.is_crouching = true;
            return;
        }

        if !self.is_crouching {
            return;
        }

        let delta = PLAYER_EYE_TO_FEET_STANDING - PLAYER_EYE_TO_FEET_CROUCHING;
        let standing_candidate = self.camera.position + Vec3::Y * delta;
        if self.is_camera_position_walkable_with_eye_to_feet(
            standing_candidate,
            PLAYER_EYE_TO_FEET_STANDING,
            is_solid,
        ) {
            self.camera.position = standing_candidate;
            self.is_crouching = false;
        }
    }

    fn is_standing_on_solid_ground<F>(&self, position: Vec3, is_solid: &mut F) -> bool
    where
        F: FnMut(IVec3) -> bool,
    {
        let eye_to_feet = self.current_eye_to_feet();
        for offset in Self::collision_offsets() {
            let ground_probe =
                position + offset - Vec3::Y * (eye_to_feet + PLAYER_GROUND_PROBE_EPSILON);
            if is_solid(ground_probe.floor().as_ivec3()) {
                return true;
            }
        }

        false
    }

    fn move_camera_vertically_with_collision<F>(&mut self, delta_y: f32, is_solid: &mut F)
    where
        F: FnMut(IVec3) -> bool,
    {
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
            if self.is_camera_position_walkable(candidate, is_solid) {
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

    fn apply_block_collision_to_camera_movement<F>(
        &mut self,
        previous_position: Vec3,
        is_solid: &mut F,
    ) where
        F: FnMut(IVec3) -> bool,
    {
        if !self.spawn_aligned_to_world {
            return;
        }

        let desired = self.camera.position;
        self.camera.position = previous_position;

        let x_candidate = Vec3::new(desired.x, self.camera.position.y, self.camera.position.z);
        if self.is_camera_position_walkable(x_candidate, is_solid) {
            self.camera.position.x = x_candidate.x;
        }

        let z_candidate = Vec3::new(self.camera.position.x, self.camera.position.y, desired.z);
        if self.is_camera_position_walkable(z_candidate, is_solid) {
            self.camera.position.z = z_candidate.z;
        }

        if !self.is_camera_position_walkable(self.camera.position, is_solid) {
            self.camera.position = previous_position;
        }
    }
}

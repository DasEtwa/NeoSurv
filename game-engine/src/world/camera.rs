use glam::{Mat4, Vec2, Vec3};

const WORLD_UP: Vec3 = Vec3::Y;
const MAX_PITCH_DEGREES: f32 = 89.0;
const CAMERA_FOV_Y_DEGREES: f32 = 60.0;
const CAMERA_Z_NEAR: f32 = 0.1;
const CAMERA_Z_FAR: f32 = 200.0;

#[derive(Debug, Clone)]
pub(crate) struct Camera {
    pub(crate) position: Vec3,
    /// Yaw angle in degrees.
    pub(crate) yaw: f32,
    /// Pitch angle in degrees.
    pub(crate) pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 24.0, 3.0),
            yaw: -90.0,
            pitch: 0.0,
        }
    }
}

impl Camera {
    pub(crate) fn forward(&self) -> Vec3 {
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();

        Vec3::new(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        )
        .normalize_or_zero()
    }

    pub(crate) fn right(&self) -> Vec3 {
        self.forward().cross(WORLD_UP).normalize_or_zero()
    }

    pub(crate) fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.position + self.forward(), WORLD_UP)
    }

    pub(crate) fn projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        let aspect_ratio = aspect_ratio.max(0.0001);
        Mat4::perspective_rh(
            CAMERA_FOV_Y_DEGREES.to_radians(),
            aspect_ratio,
            CAMERA_Z_NEAR,
            CAMERA_Z_FAR,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CameraController {
    pub(crate) move_speed: f32,
    pub(crate) mouse_sensitivity: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            move_speed: 5.0,
            mouse_sensitivity: 0.12,
        }
    }
}

impl CameraController {
    pub(crate) fn update(&self, camera: &mut Camera, move_axis: Vec3, mouse_delta: Vec2, dt: f32) {
        camera.yaw += mouse_delta.x * self.mouse_sensitivity;
        camera.pitch -= mouse_delta.y * self.mouse_sensitivity;
        camera.pitch = camera.pitch.clamp(-MAX_PITCH_DEGREES, MAX_PITCH_DEGREES);

        let forward = camera.forward();
        let right = camera.right();
        let planar_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();

        let movement = (planar_forward * -move_axis.z + right * move_axis.x).normalize_or_zero()
            * self.move_speed
            * dt;

        camera.position += movement;
    }
}

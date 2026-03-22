use glam::Vec3;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Transform {
    pub(crate) position: Vec3,
    pub(crate) rotation: Vec3,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneEntity {
    pub(crate) id: u64,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Velocity {
    pub(crate) linear: Vec3,
}

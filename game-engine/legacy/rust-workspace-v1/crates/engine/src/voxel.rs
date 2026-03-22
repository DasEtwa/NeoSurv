use glam::IVec3;

pub const CHUNK_SIZE: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Air,
    Dirt,
    Grass,
    Stone,
}

#[derive(Debug, Clone, Copy)]
pub struct Voxel {
    pub pos: IVec3,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub origin: IVec3,
    pub voxels: Vec<Voxel>,
}

#[derive(Debug, Clone, Copy)]
pub struct Quad {
    pub a: IVec3,
    pub b: IVec3,
    pub c: IVec3,
    pub d: IVec3,
}

pub fn greedy_mesh(_chunk: &Chunk) -> Vec<Quad> {
    // Placeholder: hier kommt später der echte greedy meshing pass rein.
    Vec::new()
}

pub fn raycast_block(_origin: glam::Vec3, _dir: glam::Vec3, _max_dist: f32) -> Option<IVec3> {
    // Placeholder für Block-Picking im Voxel-Modus.
    None
}

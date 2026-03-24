use glam::IVec3;

use crate::world::voxel::{
    block::BlockType,
    chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, ChunkData, LocalCoord},
};

const FACE_AREA_YZ: usize = (CHUNK_SIZE_Y as usize) * (CHUNK_SIZE_Z as usize);
const FACE_AREA_XZ: usize = (CHUNK_SIZE_X as usize) * (CHUNK_SIZE_Z as usize);
const FACE_AREA_XY: usize = (CHUNK_SIZE_X as usize) * (CHUNK_SIZE_Y as usize);

#[derive(Debug, Clone, Copy)]
pub(crate) struct MeshVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) material_id: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ChunkMesh {
    pub(crate) vertices: Vec<MeshVertex>,
    pub(crate) indices: Vec<u32>,
}

impl ChunkMesh {
    pub(crate) fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ChunkNeighborSolidity {
    positive_x: [bool; FACE_AREA_YZ],
    negative_x: [bool; FACE_AREA_YZ],
    positive_y: [bool; FACE_AREA_XZ],
    negative_y: [bool; FACE_AREA_XZ],
    positive_z: [bool; FACE_AREA_XY],
    negative_z: [bool; FACE_AREA_XY],
}

impl Default for ChunkNeighborSolidity {
    fn default() -> Self {
        Self {
            positive_x: [false; FACE_AREA_YZ],
            negative_x: [false; FACE_AREA_YZ],
            positive_y: [false; FACE_AREA_XZ],
            negative_y: [false; FACE_AREA_XZ],
            positive_z: [false; FACE_AREA_XY],
            negative_z: [false; FACE_AREA_XY],
        }
    }
}

impl ChunkNeighborSolidity {
    pub(crate) fn set_positive_x(&mut self, chunk: &ChunkData) {
        let x = (CHUNK_SIZE_X - 1) as u8;
        Self::populate_yz(&mut self.positive_x, chunk, x);
    }

    pub(crate) fn set_negative_x(&mut self, chunk: &ChunkData) {
        Self::populate_yz(&mut self.negative_x, chunk, 0);
    }

    pub(crate) fn set_positive_y(&mut self, chunk: &ChunkData) {
        let y = (CHUNK_SIZE_Y - 1) as u8;
        Self::populate_xz(&mut self.positive_y, chunk, y);
    }

    pub(crate) fn set_negative_y(&mut self, chunk: &ChunkData) {
        Self::populate_xz(&mut self.negative_y, chunk, 0);
    }

    pub(crate) fn set_positive_z(&mut self, chunk: &ChunkData) {
        let z = (CHUNK_SIZE_Z - 1) as u8;
        Self::populate_xy(&mut self.positive_z, chunk, z);
    }

    pub(crate) fn set_negative_z(&mut self, chunk: &ChunkData) {
        Self::populate_xy(&mut self.negative_z, chunk, 0);
    }

    fn populate_yz(target: &mut [bool; FACE_AREA_YZ], chunk: &ChunkData, x: u8) {
        for y in 0..CHUNK_SIZE_Y as u8 {
            for z in 0..CHUNK_SIZE_Z as u8 {
                let local = LocalCoord::new(x, y, z);
                target[Self::yz_index(y as i32, z as i32)] = chunk.block(local).is_solid();
            }
        }
    }

    fn populate_xz(target: &mut [bool; FACE_AREA_XZ], chunk: &ChunkData, y: u8) {
        for x in 0..CHUNK_SIZE_X as u8 {
            for z in 0..CHUNK_SIZE_Z as u8 {
                let local = LocalCoord::new(x, y, z);
                target[Self::xz_index(x as i32, z as i32)] = chunk.block(local).is_solid();
            }
        }
    }

    fn populate_xy(target: &mut [bool; FACE_AREA_XY], chunk: &ChunkData, z: u8) {
        for y in 0..CHUNK_SIZE_Y as u8 {
            for x in 0..CHUNK_SIZE_X as u8 {
                let local = LocalCoord::new(x, y, z);
                target[Self::xy_index(x as i32, y as i32)] = chunk.block(local).is_solid();
            }
        }
    }

    fn sample_out_of_bounds(&self, local: IVec3) -> bool {
        if Self::in_bounds(local.y, CHUNK_SIZE_Y) && Self::in_bounds(local.z, CHUNK_SIZE_Z) {
            if local.x == CHUNK_SIZE_X {
                return self.positive_x[Self::yz_index(local.y, local.z)];
            }
            if local.x == -1 {
                return self.negative_x[Self::yz_index(local.y, local.z)];
            }
        }

        if Self::in_bounds(local.x, CHUNK_SIZE_X) && Self::in_bounds(local.z, CHUNK_SIZE_Z) {
            if local.y == CHUNK_SIZE_Y {
                return self.positive_y[Self::xz_index(local.x, local.z)];
            }
            if local.y == -1 {
                return self.negative_y[Self::xz_index(local.x, local.z)];
            }
        }

        if Self::in_bounds(local.x, CHUNK_SIZE_X) && Self::in_bounds(local.y, CHUNK_SIZE_Y) {
            if local.z == CHUNK_SIZE_Z {
                return self.positive_z[Self::xy_index(local.x, local.y)];
            }
            if local.z == -1 {
                return self.negative_z[Self::xy_index(local.x, local.y)];
            }
        }

        false
    }

    fn in_bounds(value: i32, max: i32) -> bool {
        value >= 0 && value < max
    }

    fn yz_index(y: i32, z: i32) -> usize {
        (y as usize * CHUNK_SIZE_Z as usize) + z as usize
    }

    fn xz_index(x: i32, z: i32) -> usize {
        (x as usize * CHUNK_SIZE_Z as usize) + z as usize
    }

    fn xy_index(x: i32, y: i32) -> usize {
        (y as usize * CHUNK_SIZE_X as usize) + x as usize
    }
}

#[derive(Debug, Clone, Copy)]
enum FaceDir {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl FaceDir {
    fn normal(self) -> [f32; 3] {
        match self {
            Self::PosX => [1.0, 0.0, 0.0],
            Self::NegX => [-1.0, 0.0, 0.0],
            Self::PosY => [0.0, 1.0, 0.0],
            Self::NegY => [0.0, -1.0, 0.0],
            Self::PosZ => [0.0, 0.0, 1.0],
            Self::NegZ => [0.0, 0.0, -1.0],
        }
    }

    fn neighbor_delta(self) -> IVec3 {
        match self {
            Self::PosX => IVec3::new(1, 0, 0),
            Self::NegX => IVec3::new(-1, 0, 0),
            Self::PosY => IVec3::new(0, 1, 0),
            Self::NegY => IVec3::new(0, -1, 0),
            Self::PosZ => IVec3::new(0, 0, 1),
            Self::NegZ => IVec3::new(0, 0, -1),
        }
    }

    /// Returns (slice_count, axis_a_count, axis_b_count)
    fn dimensions(self) -> (i32, i32, i32) {
        match self {
            Self::PosX | Self::NegX => (CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z),
            Self::PosY | Self::NegY => (CHUNK_SIZE_Y, CHUNK_SIZE_X, CHUNK_SIZE_Z),
            Self::PosZ | Self::NegZ => (CHUNK_SIZE_Z, CHUNK_SIZE_Y, CHUNK_SIZE_X),
        }
    }

    /// Maps (slice, a, b) coordinates into local voxel-space x/y/z.
    fn local_coord(self, slice: i32, a: i32, b: i32) -> IVec3 {
        match self {
            Self::PosX | Self::NegX => IVec3::new(slice, a, b),
            Self::PosY | Self::NegY => IVec3::new(a, slice, b),
            Self::PosZ | Self::NegZ => IVec3::new(b, a, slice),
        }
    }

    fn texture_tile(self, block: BlockType) -> u32 {
        match block {
            BlockType::Grass => match self {
                Self::PosY => 0,
                Self::NegY => 2,
                _ => 1,
            },
            BlockType::Stone => 3,
            BlockType::Sand => 4,
            BlockType::Dirt => 2,
            BlockType::BorderWall => 3,
            BlockType::Dummy => 4,
            BlockType::Air => 3,
        }
    }
}

const FACE_DIRS: [FaceDir; 6] = [
    FaceDir::PosX,
    FaceDir::NegX,
    FaceDir::PosY,
    FaceDir::NegY,
    FaceDir::PosZ,
    FaceDir::NegZ,
];

pub(crate) fn build_chunk_mesh(chunk: &ChunkData) -> ChunkMesh {
    build_chunk_mesh_with_neighbors(chunk, &ChunkNeighborSolidity::default())
}

pub(crate) fn build_chunk_mesh_with_neighbors(
    chunk: &ChunkData,
    neighbors: &ChunkNeighborSolidity,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::default();
    let chunk_origin = chunk.coord.origin_world();

    for face in FACE_DIRS {
        mesh_face_direction(chunk, neighbors, chunk_origin, face, &mut mesh);
    }

    mesh
}

fn mesh_face_direction(
    chunk: &ChunkData,
    neighbors: &ChunkNeighborSolidity,
    chunk_origin: IVec3,
    face: FaceDir,
    mesh: &mut ChunkMesh,
) {
    let (slice_count, a_count, b_count) = face.dimensions();
    let mut mask = vec![None; (a_count * b_count) as usize];

    for slice in 0..slice_count {
        for b in 0..b_count {
            for a in 0..a_count {
                let local = face.local_coord(slice, a, b);
                let block = block_at(chunk, local);
                let index = mask_index(a_count, a, b);

                if !block.is_solid() {
                    mask[index] = None;
                    continue;
                }

                let neighbor =
                    block_at_with_neighbors(chunk, neighbors, local + face.neighbor_delta());
                mask[index] = (!neighbor.is_solid()).then_some(face.texture_tile(block));
            }
        }

        greedy_merge_mask(
            &mut mask,
            a_count,
            b_count,
            |a, b, a_len, b_len, material_id| {
                let local = face.local_coord(slice, a, b);
                let base_world = chunk_origin + local;
                append_merged_face(mesh, face, base_world, a_len, b_len, material_id);
            },
        );
    }
}

fn greedy_merge_mask(
    mask: &mut [Option<u32>],
    a_count: i32,
    b_count: i32,
    mut emit: impl FnMut(i32, i32, i32, i32, u32),
) {
    for b in 0..b_count {
        let mut a = 0;

        while a < a_count {
            let index = mask_index(a_count, a, b);
            let Some(material_id) = mask[index] else {
                a += 1;
                continue;
            };

            let mut width = 1;
            while a + width < a_count
                && mask[mask_index(a_count, a + width, b)] == Some(material_id)
            {
                width += 1;
            }

            let mut height = 1;
            'height: while b + height < b_count {
                for da in 0..width {
                    if mask[mask_index(a_count, a + da, b + height)] != Some(material_id) {
                        break 'height;
                    }
                }

                height += 1;
            }

            for db in 0..height {
                for da in 0..width {
                    let clear_index = mask_index(a_count, a + da, b + db);
                    mask[clear_index] = None;
                }
            }

            emit(a, b, width, height, material_id);
            a += width;
        }
    }
}

fn append_merged_face(
    mesh: &mut ChunkMesh,
    face: FaceDir,
    base_world: IVec3,
    a_len: i32,
    b_len: i32,
    material_id: u32,
) {
    let x = base_world.x as f32;
    let y = base_world.y as f32;
    let z = base_world.z as f32;
    let a = a_len as f32;
    let b = b_len as f32;

    let corners = match face {
        // axis-a = +Y, axis-b = +Z
        FaceDir::PosX => [
            [x + 1.0, y, z],
            [x + 1.0, y + a, z],
            [x + 1.0, y + a, z + b],
            [x + 1.0, y, z + b],
        ],
        // axis-a = +Y, axis-b = +Z (winding adjusted for outward -X)
        FaceDir::NegX => [[x, y, z + b], [x, y + a, z + b], [x, y + a, z], [x, y, z]],
        // axis-a = +X, axis-b = +Z (winding adjusted for outward +Y)
        FaceDir::PosY => [
            [x, y + 1.0, z + b],
            [x + a, y + 1.0, z + b],
            [x + a, y + 1.0, z],
            [x, y + 1.0, z],
        ],
        // axis-a = +X, axis-b = +Z
        FaceDir::NegY => [[x, y, z], [x + a, y, z], [x + a, y, z + b], [x, y, z + b]],
        // axis-a = +Y, axis-b = +X (winding adjusted for outward +Z)
        FaceDir::PosZ => [
            [x + b, y, z + 1.0],
            [x + b, y + a, z + 1.0],
            [x, y + a, z + 1.0],
            [x, y, z + 1.0],
        ],
        // axis-a = +Y, axis-b = +X
        FaceDir::NegZ => [[x, y, z], [x, y + a, z], [x + b, y + a, z], [x + b, y, z]],
    };

    append_quad(mesh, corners, face.normal(), a, b, material_id);
}

fn append_quad(
    mesh: &mut ChunkMesh,
    corners: [[f32; 3]; 4],
    normal: [f32; 3],
    a_len: f32,
    b_len: f32,
    material_id: u32,
) {
    let base_index = mesh.vertices.len() as u32;
    let uv = [[0.0, 0.0], [0.0, a_len], [b_len, a_len], [b_len, 0.0]];

    for (position, uv) in corners.into_iter().zip(uv) {
        mesh.vertices.push(MeshVertex {
            position,
            normal,
            uv,
            material_id,
        });
    }

    mesh.indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}

fn block_at(chunk: &ChunkData, local: IVec3) -> BlockType {
    LocalCoord::try_from_ivec3(local)
        .map(|coord| chunk.block(coord))
        .unwrap_or(BlockType::Air)
}

fn block_at_with_neighbors(
    chunk: &ChunkData,
    neighbors: &ChunkNeighborSolidity,
    local: IVec3,
) -> BlockType {
    if let Some(coord) = LocalCoord::try_from_ivec3(local) {
        return chunk.block(coord);
    }

    if neighbors.sample_out_of_bounds(local) {
        BlockType::Stone
    } else {
        BlockType::Air
    }
}

fn mask_index(a_count: i32, a: i32, b: i32) -> usize {
    (b * a_count + a) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::voxel::chunk::ChunkCoord;

    fn quad_count(mesh: &ChunkMesh) -> usize {
        mesh.indices.len() / 6
    }

    fn assert_indices_in_bounds(mesh: &ChunkMesh) {
        assert!(
            mesh.indices
                .iter()
                .all(|index| (*index as usize) < mesh.vertices.len())
        );
    }

    #[test]
    fn mesher_outputs_cube_surface_only() {
        let mut chunk = ChunkData::new(ChunkCoord::new(0, 0, 0));
        chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);

        let mesh = build_chunk_mesh(&chunk);

        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
        assert_indices_in_bounds(&mesh);
    }

    #[test]
    fn greedy_merges_rectangular_prism_into_six_quads() {
        let mut chunk = ChunkData::new(ChunkCoord::new(0, 0, 0));
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    chunk.set_block(LocalCoord::new(x, y, z), BlockType::Stone);
                }
            }
        }

        let mesh = build_chunk_mesh(&chunk);

        assert_eq!(quad_count(&mesh), 6);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
        assert_indices_in_bounds(&mesh);
    }

    #[test]
    fn greedy_preserves_internal_cavity_faces() {
        let mut chunk = ChunkData::new(ChunkCoord::new(0, 0, 0));

        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    chunk.set_block(LocalCoord::new(x, y, z), BlockType::Stone);
                }
            }
        }

        chunk.set_block(LocalCoord::new(1, 1, 1), BlockType::Air);

        let mesh = build_chunk_mesh(&chunk);

        // 3x3x3 shell with center air:
        // - outer surface greedily merges to 6 quads
        // - inner cavity contributes 6 additional quads
        assert_eq!(quad_count(&mesh), 12);
        assert_indices_in_bounds(&mesh);
    }

    #[test]
    fn greedy_does_not_merge_across_hole() {
        let mut chunk = ChunkData::new(ChunkCoord::new(0, 0, 0));

        for x in 0..3 {
            for z in 0..3 {
                if x == 1 && z == 1 {
                    continue;
                }

                chunk.set_block(LocalCoord::new(x, 0, z), BlockType::Stone);
            }
        }

        let mesh = build_chunk_mesh(&chunk);

        // 3x3 plate with center hole at y=0.
        // Top and bottom each split into 4 quads around the hole, plus
        // 4 outer side quads and 4 inner-hole side quads.
        assert_eq!(quad_count(&mesh), 16);
        assert_indices_in_bounds(&mesh);
    }

    #[test]
    fn mesher_output_is_deterministic_for_same_chunk_data() {
        let mut chunk = ChunkData::new(ChunkCoord::new(0, 0, 0));

        let blocks = [
            LocalCoord::new(0, 0, 0),
            LocalCoord::new(1, 0, 0),
            LocalCoord::new(1, 1, 0),
            LocalCoord::new(2, 1, 1),
            LocalCoord::new(2, 2, 1),
            LocalCoord::new(3, 2, 2),
        ];

        for local in blocks {
            chunk.set_block(local, BlockType::Stone);
        }

        let first = build_chunk_mesh(&chunk);
        let second = build_chunk_mesh(&chunk);

        assert_eq!(first.indices, second.indices);
        assert_eq!(first.vertices.len(), second.vertices.len());

        for (a, b) in first.vertices.iter().zip(second.vertices.iter()) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.normal, b.normal);
            assert_eq!(a.uv, b.uv);
            assert_eq!(a.material_id, b.material_id);
        }
    }

    #[test]
    fn mesher_culls_boundary_faces_against_adjacent_solid_chunk() {
        let mut left = ChunkData::new(ChunkCoord::new(0, 0, 0));
        let mut right = ChunkData::new(ChunkCoord::new(1, 0, 0));
        left.fill(BlockType::Stone);
        right.fill(BlockType::Stone);

        let no_neighbor_mesh = build_chunk_mesh(&left);

        let mut neighbors = ChunkNeighborSolidity::default();
        neighbors.set_positive_x(&right);
        let neighbor_aware_mesh = build_chunk_mesh_with_neighbors(&left, &neighbors);

        assert_eq!(quad_count(&no_neighbor_mesh), 6);
        assert_eq!(quad_count(&neighbor_aware_mesh), 5);
        assert_indices_in_bounds(&neighbor_aware_mesh);
    }

    #[test]
    fn two_adjacent_solid_chunks_do_not_emit_internal_boundary_faces() {
        let mut left = ChunkData::new(ChunkCoord::new(0, 0, 0));
        let mut right = ChunkData::new(ChunkCoord::new(1, 0, 0));
        left.fill(BlockType::Stone);
        right.fill(BlockType::Stone);

        let left_without_neighbors = build_chunk_mesh(&left);
        let right_without_neighbors = build_chunk_mesh(&right);

        let mut left_neighbors = ChunkNeighborSolidity::default();
        left_neighbors.set_positive_x(&right);
        let left_with_neighbors = build_chunk_mesh_with_neighbors(&left, &left_neighbors);

        let mut right_neighbors = ChunkNeighborSolidity::default();
        right_neighbors.set_negative_x(&left);
        let right_with_neighbors = build_chunk_mesh_with_neighbors(&right, &right_neighbors);

        assert_eq!(quad_count(&left_without_neighbors), 6);
        assert_eq!(quad_count(&right_without_neighbors), 6);

        // One internal boundary quad is removed from each chunk.
        assert_eq!(quad_count(&left_with_neighbors), 5);
        assert_eq!(quad_count(&right_with_neighbors), 5);

        assert_eq!(
            quad_count(&left_without_neighbors) + quad_count(&right_without_neighbors),
            quad_count(&left_with_neighbors) + quad_count(&right_with_neighbors) + 2
        );
    }
}

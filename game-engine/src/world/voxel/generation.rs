use noise::{NoiseFn, OpenSimplex};

use crate::world::voxel::{
    block::BlockType,
    chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, ChunkCoord, ChunkData, LocalCoord},
};

#[derive(Debug, Clone)]
pub(crate) struct TerrainGenerator {
    base_height: i32,
    terrain_amplitude: i32,
    sea_level: i32,
    macro_noise: OpenSimplex,
    detail_noise: OpenSimplex,
}

impl TerrainGenerator {
    pub(crate) fn new(seed: u32) -> Self {
        Self {
            base_height: 8,
            terrain_amplitude: 16,
            sea_level: 4,
            macro_noise: OpenSimplex::new(seed),
            detail_noise: OpenSimplex::new(seed ^ 0xA53A_5A3A),
        }
    }

    pub(crate) fn generate_chunk(&self, coord: ChunkCoord) -> ChunkData {
        let mut chunk = ChunkData::new(coord);
        let chunk_origin = coord.origin_world();

        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let world_x = chunk_origin.x + x;
                    let world_y = chunk_origin.y + y;
                    let world_z = chunk_origin.z + z;

                    let surface_height = self.surface_height(world_x, world_z);
                    let block = self.block_for_height(world_y, surface_height);

                    chunk.set_block(LocalCoord::new(x as u8, y as u8, z as u8), block);
                }
            }
        }

        chunk
    }

    fn surface_height(&self, world_x: i32, world_z: i32) -> i32 {
        let macro_sample = self
            .macro_noise
            .get([world_x as f64 * 0.01, world_z as f64 * 0.01]);
        let detail_sample = self
            .detail_noise
            .get([world_x as f64 * 0.05, world_z as f64 * 0.05]);

        let height = self.base_height as f64
            + macro_sample * self.terrain_amplitude as f64
            + detail_sample * (self.terrain_amplitude as f64 * 0.25);

        height.round() as i32
    }

    fn block_for_height(&self, world_y: i32, surface_height: i32) -> BlockType {
        if world_y > surface_height {
            return BlockType::Air;
        }

        if world_y == surface_height {
            if world_y <= self.sea_level {
                return BlockType::Sand;
            }
            return BlockType::Grass;
        }

        if world_y >= surface_height - 3 {
            return BlockType::Dirt;
        }

        BlockType::Stone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_per_seed() {
        let a = TerrainGenerator::new(42).generate_chunk(ChunkCoord::new(0, 0, 0));
        let b = TerrainGenerator::new(42).generate_chunk(ChunkCoord::new(0, 0, 0));
        let c = TerrainGenerator::new(7).generate_chunk(ChunkCoord::new(0, 0, 0));

        let sample_points = [
            LocalCoord::new(0, 0, 0),
            LocalCoord::new(3, 6, 9),
            LocalCoord::new(8, 8, 8),
            LocalCoord::new(12, 4, 2),
            LocalCoord::new(15, 15, 15),
        ];

        for point in sample_points {
            assert_eq!(a.block(point), b.block(point));
        }

        let differs = sample_points
            .iter()
            .copied()
            .any(|point| a.block(point) != c.block(point));
        assert!(differs, "different seed should change terrain samples");
    }
}

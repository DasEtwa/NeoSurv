use glam::IVec3;

use crate::world::voxel::block::BlockType;

pub(crate) const CHUNK_SIZE_X: i32 = 16;
pub(crate) const CHUNK_SIZE_Y: i32 = 16;
pub(crate) const CHUNK_SIZE_Z: i32 = 16;

const CHUNK_VOLUME: usize =
    (CHUNK_SIZE_X as usize) * (CHUNK_SIZE_Y as usize) * (CHUNK_SIZE_Z as usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ChunkCoord {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
}

impl ChunkCoord {
    pub(crate) const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn from_world(world: IVec3) -> Self {
        Self {
            x: world.x.div_euclid(CHUNK_SIZE_X),
            y: world.y.div_euclid(CHUNK_SIZE_Y),
            z: world.z.div_euclid(CHUNK_SIZE_Z),
        }
    }

    pub(crate) fn origin_world(self) -> IVec3 {
        IVec3::new(
            self.x * CHUNK_SIZE_X,
            self.y * CHUNK_SIZE_Y,
            self.z * CHUNK_SIZE_Z,
        )
    }

    pub(crate) fn world_from_local(self, local: LocalCoord) -> IVec3 {
        self.origin_world() + local.as_ivec3()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LocalCoord {
    pub(crate) x: u8,
    pub(crate) y: u8,
    pub(crate) z: u8,
}

impl LocalCoord {
    pub(crate) const fn new(x: u8, y: u8, z: u8) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn as_ivec3(self) -> IVec3 {
        IVec3::new(self.x as i32, self.y as i32, self.z as i32)
    }

    pub(crate) fn try_from_ivec3(value: IVec3) -> Option<Self> {
        if value.x < 0
            || value.x >= CHUNK_SIZE_X
            || value.y < 0
            || value.y >= CHUNK_SIZE_Y
            || value.z < 0
            || value.z >= CHUNK_SIZE_Z
        {
            return None;
        }

        Some(Self {
            x: value.x as u8,
            y: value.y as u8,
            z: value.z as u8,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkData {
    pub(crate) coord: ChunkCoord,
    blocks: Vec<BlockType>,
}

impl ChunkData {
    pub(crate) fn new(coord: ChunkCoord) -> Self {
        Self {
            coord,
            blocks: vec![BlockType::Air; CHUNK_VOLUME],
        }
    }

    pub(crate) fn block(&self, local: LocalCoord) -> BlockType {
        self.blocks[Self::index(local)]
    }

    pub(crate) fn set_block(&mut self, local: LocalCoord, block: BlockType) {
        let index = Self::index(local);
        self.blocks[index] = block;
    }

    pub(crate) fn fill(&mut self, block: BlockType) {
        self.blocks.fill(block);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.blocks.iter().all(|block| !block.is_solid())
    }

    pub(crate) fn world_to_local(world: IVec3) -> LocalCoord {
        LocalCoord {
            x: world.x.rem_euclid(CHUNK_SIZE_X) as u8,
            y: world.y.rem_euclid(CHUNK_SIZE_Y) as u8,
            z: world.z.rem_euclid(CHUNK_SIZE_Z) as u8,
        }
    }

    pub(crate) fn block_world(&self, world: IVec3) -> Option<BlockType> {
        let coord = ChunkCoord::from_world(world);
        if coord != self.coord {
            return None;
        }

        Some(self.block(Self::world_to_local(world)))
    }

    pub(crate) fn iter_solid_blocks(&self) -> impl Iterator<Item = (LocalCoord, BlockType)> + '_ {
        self.blocks
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, block)| block.is_solid())
            .map(|(index, block)| (Self::local_from_index(index), block))
    }

    pub(crate) fn volume() -> usize {
        CHUNK_VOLUME
    }

    fn index(local: LocalCoord) -> usize {
        (local.y as usize * CHUNK_SIZE_Z as usize + local.z as usize) * CHUNK_SIZE_X as usize
            + local.x as usize
    }

    fn local_from_index(index: usize) -> LocalCoord {
        let x = index % CHUNK_SIZE_X as usize;
        let yz = index / CHUNK_SIZE_X as usize;
        let z = yz % CHUNK_SIZE_Z as usize;
        let y = yz / CHUNK_SIZE_Z as usize;

        LocalCoord::new(x as u8, y as u8, z as u8)
    }
}

pub(crate) fn split_world_position(world: IVec3) -> (ChunkCoord, LocalCoord) {
    let chunk = ChunkCoord::from_world(world);
    let local = ChunkData::world_to_local(world);
    (chunk, local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_world_roundtrip_handles_negative_space() {
        let samples = [
            IVec3::new(0, 0, 0),
            IVec3::new(15, 15, 15),
            IVec3::new(16, 0, 0),
            IVec3::new(-1, -1, -1),
            IVec3::new(-16, -16, -16),
            IVec3::new(-17, 31, 33),
        ];

        for world in samples {
            let (chunk, local) = split_world_position(world);
            let rebuilt = chunk.world_from_local(local);
            assert_eq!(rebuilt, world);
        }
    }

    #[test]
    fn chunk_index_roundtrip() {
        let mut chunk = ChunkData::new(ChunkCoord::new(0, 0, 0));
        chunk.fill(BlockType::Air);

        let local = LocalCoord::new(4, 9, 13);
        chunk.set_block(local, BlockType::Stone);

        assert_eq!(chunk.block(local), BlockType::Stone);
        assert_eq!(chunk.iter_solid_blocks().count(), 1);
    }
}

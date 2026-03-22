use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub(crate) enum BlockType {
    #[default]
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    BorderWall = 5,
    Dummy = 6,
}

impl BlockType {
    pub(crate) fn is_solid(self) -> bool {
        !matches!(self, Self::Air)
    }

    pub(crate) fn material_id(self) -> u32 {
        self as u32
    }
}

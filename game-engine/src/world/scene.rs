use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EntityRecord {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) position: Vec3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Scene {
    pub(crate) name: String,
    pub(crate) entities: Vec<EntityRecord>,
}

impl Scene {
    pub(crate) fn demo() -> Self {
        Self {
            name: "demo".to_owned(),
            entities: vec![EntityRecord {
                id: 1,
                name: "player".to_owned(),
                position: Vec3::new(0.0, 1.8, 0.0),
            }],
        }
    }
}

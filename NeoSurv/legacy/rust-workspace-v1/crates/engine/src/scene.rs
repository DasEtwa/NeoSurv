use anyhow::Result;
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntity {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub entities: Vec<SceneEntity>,
}

impl Scene {
    pub fn default_demo() -> Self {
        Self {
            name: "DemoScene".to_string(),
            entities: vec![SceneEntity {
                id: 1,
                name: "Player".to_string(),
                position: Vec3::new(0.0, 2.0, 0.0),
            }],
        }
    }
}

pub fn to_ron_pretty(scene: &Scene) -> Result<String> {
    let cfg = ron::ser::PrettyConfig::new();
    Ok(ron::ser::to_string_pretty(scene, cfg)?)
}

pub fn from_ron(input: &str) -> Result<Scene> {
    Ok(ron::from_str(input)?)
}

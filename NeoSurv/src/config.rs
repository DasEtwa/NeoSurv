use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GraphicsBackend {
    Vulkan,
    Opengl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GraphicsConfig {
    pub(crate) backend: GraphicsBackend,
    pub(crate) vsync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WindowConfig {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InputConfig {
    pub(crate) mouse_sensitivity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppConfig {
    pub(crate) graphics: GraphicsConfig,
    pub(crate) window: WindowConfig,
    pub(crate) input: InputConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            graphics: GraphicsConfig {
                backend: GraphicsBackend::Vulkan,
                vsync: true,
            },
            window: WindowConfig {
                width: 1280,
                height: 720,
                title: "Tokenburner".to_string(),
            },
            input: InputConfig {
                mouse_sensitivity: 0.12,
            },
        }
    }
}

impl AppConfig {
    pub(crate) fn load_or_default(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str::<Self>(&text).ok())
            .unwrap_or_default()
    }
}

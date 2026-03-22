#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererBackend {
    Vulkan,
    OpenGl,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub app_name: String,
    pub renderer: RendererBackend,
    pub target_fps: u32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            app_name: "MaxEngine".to_string(),
            renderer: RendererBackend::Vulkan,
            target_fps: 60,
        }
    }
}

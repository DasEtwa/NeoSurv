use crate::config::RendererBackend;
use tracing::debug;

pub trait Renderer {
    fn begin_frame(&mut self, frame: u64);
    fn end_frame(&mut self, frame: u64);
    fn backend_name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct WgpuRenderer {
    backend: RendererBackend,
}

impl WgpuRenderer {
    pub fn new(backend: RendererBackend) -> Self {
        Self { backend }
    }
}

impl Renderer for WgpuRenderer {
    fn begin_frame(&mut self, frame: u64) {
        debug!(frame, backend = self.backend_name(), "begin frame");
    }

    fn end_frame(&mut self, frame: u64) {
        debug!(frame, backend = self.backend_name(), "end frame");
    }

    fn backend_name(&self) -> &'static str {
        match self.backend {
            RendererBackend::Vulkan => "Vulkan (wgpu)",
            RendererBackend::OpenGl => "OpenGL (wgpu)",
        }
    }
}

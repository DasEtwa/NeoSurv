use anyhow::Result;
use winit::{dpi::PhysicalSize, window::Window};

use super::CameraMatrices;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ClearColor {
    pub(crate) r: f64,
    pub(crate) g: f64,
    pub(crate) b: f64,
    pub(crate) a: f64,
}

impl ClearColor {
    pub(crate) const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
}

pub(crate) trait Backend {
    fn name(&self) -> &'static str;
    fn resize(&mut self, size: PhysicalSize<u32>);
    fn update_camera_matrices(&mut self, camera: CameraMatrices);
    fn render(&mut self, clear: ClearColor) -> Result<()>;
    fn request_redraw(&self, window: &Window) {
        window.request_redraw();
    }
}

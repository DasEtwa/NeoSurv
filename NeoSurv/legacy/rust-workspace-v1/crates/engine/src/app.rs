use std::{thread, time::Duration};

use tracing::info;

use crate::{
    config::EngineConfig,
    render::{Renderer, WgpuRenderer},
};

pub struct Engine {
    config: EngineConfig,
    renderer: WgpuRenderer,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        let renderer = WgpuRenderer::new(config.renderer);
        Self { config, renderer }
    }

    pub fn run_for_frames(&mut self, max_frames: u64) {
        let frame_time = Duration::from_secs_f64(1.0 / self.config.target_fps as f64);

        info!(
            app = %self.config.app_name,
            fps = self.config.target_fps,
            backend = self.renderer.backend_name(),
            "engine start"
        );

        for frame in 0..max_frames {
            self.renderer.begin_frame(frame);

            // TODO: scheduler.run();
            // TODO: world.update();
            // TODO: render graph execute;

            self.renderer.end_frame(frame);
            thread::sleep(frame_time);
        }

        info!("engine shutdown");
    }
}

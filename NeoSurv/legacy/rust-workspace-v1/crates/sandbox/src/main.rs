use engine::{
    app::Engine,
    config::{EngineConfig, RendererBackend},
    ecs, scene, worldgen,
};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let _world = ecs::demo_world();
    let terrain = worldgen::terrain_height(42, 128.0, 256.0);

    let scene = scene::Scene::default_demo();
    let scene_ron = scene::to_ron_pretty(&scene).expect("scene should serialize");

    info!(height = terrain, "sample terrain height generated");
    info!("scene as RON:\n{}", scene_ron);

    let mut engine = Engine::new(EngineConfig {
        app_name: "MaxEngine Sandbox".into(),
        renderer: RendererBackend::Vulkan,
        target_fps: 60,
    });

    engine.run_for_frames(120);
}

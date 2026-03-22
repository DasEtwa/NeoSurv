mod config;
mod ecs;
mod editor;
mod engine;
mod input;
mod renderer;
mod world;

use anyhow::Result;
use config::AppConfig;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let config = AppConfig::load_or_default("Config.toml");

    tracing::info!(
        backend = ?config.graphics.backend,
        "tokenburner startup"
    );

    engine::run(config)
}

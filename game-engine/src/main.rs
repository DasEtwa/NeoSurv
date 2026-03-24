mod config;
mod editor;
mod engine;
mod game;
mod input;
mod player;
mod renderer;
mod world;

use anyhow::Result;
use config::AppConfig;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
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

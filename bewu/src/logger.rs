use anyhow::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

pub fn init() -> anyhow::Result<()> {
    let env_filter = EnvFilter::default()
        .add_directive(tracing::Level::INFO.into())
        .add_directive(tracing::level_filters::LevelFilter::INFO.into());

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter)
        .try_init()
        .ok()
        .context("failed to install logger")
}

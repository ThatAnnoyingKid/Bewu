use crate::Config;
use anyhow::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

pub fn init(config: &Config) -> anyhow::Result<()> {
    let mut env_filter = EnvFilter::default()
        .add_directive(tracing::Level::INFO.into())
        .add_directive(tracing::level_filters::LevelFilter::INFO.into());

    for directive in config.logging.directives.iter() {
        let directive = directive.parse()?;
        env_filter = env_filter.add_directive(directive);
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter)
        .try_init()
        .ok()
        .context("failed to install logger")
}

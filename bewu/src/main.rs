mod app_state;
mod config;
mod logger;
mod routes;
pub mod util;

pub use self::app_state::AppState;
pub use self::config::Config;
use anyhow::Context;
use std::sync::Arc;
use tracing::error;
use tracing::info;

fn main() -> anyhow::Result<()> {
    let config = Config::load_path("config.toml").context("failed to load config")?;
    crate::logger::init().context("failed to init logger")?;

    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    tokio_rt.block_on(async_main(config))
}

async fn async_main(config: Config) -> anyhow::Result<()> {
    let app_state = Arc::new(AppState::new(&config.data_directory).await?);
    let app = self::routes::routes(&config, app_state.clone())?;
    let server =
        axum::Server::try_bind(&config.bind_address).context("failed to bind to address")?;

    info!("listening on {}", config.bind_address);

    let result = async move {
        server
            .serve(app.into_make_service())
            .with_graceful_shutdown(async {
                match tokio::signal::ctrl_c()
                    .await
                    .context("failed to register ctrl+c handler")
                {
                    Ok(()) => {
                        info!("shutting down");
                    }
                    Err(e) => {
                        error!("{e:?}");
                    }
                }
            })
            .await
            .context("server error")
    }
    .await;
    let shutdown_result = app_state
        .shutdown()
        .await
        .context("failed to shutdown app state");
    if let Err(e) = shutdown_result.as_ref() {
        error!("{e}");
    }

    result.or(shutdown_result)
}

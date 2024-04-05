mod app_state;
mod config;
mod logger;
mod routes;
pub mod util;

pub use self::app_state::AppState;
pub use self::config::Config;
use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;
use tracing::error;
use tracing::info;

fn main() -> anyhow::Result<()> {
    let config = Config::load_path("config.toml").context("failed to load config")?;
    crate::logger::init(&config).context("failed to init logger")?;

    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    tokio_rt.block_on(async_main(config))
}

async fn async_main(config: Config) -> anyhow::Result<()> {
    let app_state = Arc::new(AppState::new(&config.data_directory).await?);
    let app = self::routes::routes(&config, app_state.clone())?;
    let server_listener = tokio::net::TcpListener::bind(&config.bind_address)
        .await
        .with_context(|| format!("failed to bind to address \"{}\"", config.bind_address))?;

    info!("listening on \"{}\"", config.bind_address);

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    let mut server_task_handle = tokio::spawn(async move {
        axum::serve(server_listener, app)
            .with_graceful_shutdown(async {
                let result = tokio::signal::ctrl_c()
                    .await
                    .context("failed to register ctrl+c handler");
                let _ = shutdown_tx.send(()).is_ok();
                match result {
                    Ok(()) => {
                        info!("shutting down");
                    }
                    Err(error) => {
                        error!("{error:?}");
                    }
                }
            })
            .await
            .context("server error")
    });

    let server_result = tokio::select! {
        result = &mut server_task_handle => Some(result),
        _ = &mut shutdown_rx => None,
    };

    let server_result = match server_result {
        Some(result) => result,
        None => {
            let timeout_duration = Duration::from_secs(1);
            let timeout_future = tokio::time::sleep(timeout_duration);
            tokio::pin!(timeout_future);

            tokio::select! {
                _ = &mut timeout_future => {
                    info!("server task did not exit within {timeout_duration:?}, aborting server task");

                    server_task_handle.abort();
                    server_task_handle.await
                }
                result = &mut server_task_handle => result
            }
        }
    };

    let result = server_result
        .context("failed to join server task")
        .and_then(|r| r);

    let shutdown_result = app_state
        .shutdown()
        .await
        .context("failed to shutdown app state");

    if let Err(error) = shutdown_result.as_ref() {
        error!("{error}");
    }

    result.or(shutdown_result)
}

mod config;
mod logger;
mod routes;

pub use self::config::Config;
use anyhow::Context;
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
    let app = self::routes::routes(&config)?;
    let server =
        axum::Server::try_bind(&config.bind_address).context("failed to bind to address")?;

    info!("listening on {}", config.bind_address);

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

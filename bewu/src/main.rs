mod logger;
mod routes;

use anyhow::Context;
use std::net::SocketAddr;
use std::path::Path;
use tracing::error;
use tracing::info;

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    pub address: SocketAddr,
}

impl Config {
    pub fn load_path<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let data = std::fs::read_to_string(path)?;
        toml::from_str(&data).context("failed to parse config")
    }
}

fn main() -> anyhow::Result<()> {
    let config = Config::load_path("config.toml").context("failed to load config")?;
    crate::logger::init().context("failed to init logger")?;

    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    tokio_rt.block_on(async_main(config))
}

async fn async_main(config: Config) -> anyhow::Result<()> {
    let app = self::routes::routes()?;
    let server = axum::Server::try_bind(&config.address).context("failed to bind to address")?;

    info!("listening on {}", config.address);

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

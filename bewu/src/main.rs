use anyhow::Context;
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use tracing::error;
use tracing::info;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .try_init()
        .ok()
        .context("failed to init logger")?;

    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    tokio_rt.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    let app = Router::new().route("/", get(root));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let server = axum::Server::try_bind(&addr).context("failed to bind to address")?;

    info!("listening on {addr}");

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

async fn root() -> &'static str {
    "Hello, World!"
}

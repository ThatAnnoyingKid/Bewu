mod logger;

use anyhow::Context;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get_service;
use axum::Router;
use std::net::SocketAddr;
use std::path::Path;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tower_http::trace::DefaultMakeSpan;
use tower_http::trace::DefaultOnFailure;
use tower_http::trace::DefaultOnRequest;
use tower_http::trace::DefaultOnResponse;
use tower_http::trace::TraceLayer;
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

async fn server_error(_err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Server Error")
}

async fn async_main(config: Config) -> anyhow::Result<()> {
    let static_file_dir =
        std::fs::canonicalize("public").context("failed to canonicalize static file dir")?;

    let serve_dir =
        ServeDir::new(static_file_dir).not_found_service(ServeFile::new("public/index.html"));
    let serve_dir = get_service(serve_dir).handle_error(server_error);
    let app = Router::new().fallback_service(serve_dir).layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
            .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
            .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
            .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR)),
    );

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

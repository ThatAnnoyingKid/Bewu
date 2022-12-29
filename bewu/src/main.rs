use anyhow::Context;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::routing::get_service;
use axum::Router;
use std::net::SocketAddr;
use tower::util::ServiceExt;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tower_http::trace::DefaultMakeSpan;
use tower_http::trace::DefaultOnFailure;
use tower_http::trace::DefaultOnRequest;
use tower_http::trace::DefaultOnResponse;
use tower_http::trace::TraceLayer;
use tracing::error;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::default()
                .add_directive(tracing::Level::INFO.into())
                .add_directive(tracing::level_filters::LevelFilter::INFO.into())
                .add_directive("tower_http=debug".parse()?),
        )
        .try_init()
        .ok()
        .context("failed to init logger")?;

    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    tokio_rt.block_on(async_main())
}

async fn server_error(_err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Server Error")
}

async fn async_main() -> anyhow::Result<()> {
    let serve_dir =
        get_service(ServeDir::new("public").not_found_service(ServeFile::new("public/index.html")))
            .handle_error(server_error);
    let app = Router::new().fallback_service(serve_dir).layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
            .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
            .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
            .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR)),
    );

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
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

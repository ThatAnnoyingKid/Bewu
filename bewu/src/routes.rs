use anyhow::Context;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get_service;
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tower_http::trace::DefaultMakeSpan;
use tower_http::trace::DefaultOnFailure;
use tower_http::trace::DefaultOnRequest;
use tower_http::trace::DefaultOnResponse;
use tower_http::trace::TraceLayer;

pub fn routes() -> anyhow::Result<Router> {
    let static_file_dir =
        std::fs::canonicalize("public").context("failed to canonicalize static file dir")?;

    let serve_dir =
        ServeDir::new(static_file_dir).not_found_service(ServeFile::new("public/index.html"));
    let serve_dir = get_service(serve_dir).handle_error(server_error);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
        .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
        .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR));

    Ok(Router::new().fallback_service(serve_dir).layer(trace_layer))
}

async fn server_error(_err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Server Error")
}

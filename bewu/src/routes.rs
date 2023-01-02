mod api;

use crate::Config;

use crate::AppState;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get_service;
use axum::Router;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::trace::DefaultMakeSpan;
use tower_http::trace::DefaultOnFailure;
use tower_http::trace::DefaultOnRequest;
use tower_http::trace::DefaultOnResponse;
use tower_http::trace::TraceLayer;

pub fn routes(config: &Config, app_state: Arc<AppState>) -> anyhow::Result<Router> {
    let serve_dir = ServeDir::new(&config.public_directory)
        .not_found_service(tower::service_fn(not_found_error));
    let serve_dir = get_service(serve_dir).handle_error(server_error);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(tracing::Level::INFO)
                .include_headers(config.logging.include_headers),
        )
        .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
        .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR));

    let routes = Router::new()
        .nest("/api", self::api::routes().with_state(app_state))
        .fallback_service(serve_dir)
        .layer(trace_layer);
    Ok(routes)
}

async fn not_found_error<T>(_req: Request<T>) -> Result<Response, std::io::Error> {
    Ok((StatusCode::NOT_FOUND, "404: Not Found").into_response())
}

async fn server_error(_err: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "500: Internal Server Error",
    )
}

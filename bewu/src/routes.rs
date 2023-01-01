use crate::Config;

use axum::http::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get_service;
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::DefaultMakeSpan;
use tower_http::trace::DefaultOnFailure;
use tower_http::trace::DefaultOnRequest;
use tower_http::trace::DefaultOnResponse;
use tower_http::trace::TraceLayer;

pub fn routes(config: &Config) -> anyhow::Result<Router> {
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

    Ok(Router::new()
        .nest("/api", api_routes())
        .fallback_service(serve_dir)
        .layer(trace_layer))
}

fn api_routes() -> Router {
    Router::new()
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

use crate::AppState;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use axum::Router;
use std::sync::Arc;
use tracing::error;

#[derive(Debug, serde::Serialize)]
struct ApiError {
    messages: Vec<String>,
}

impl ApiError {
    fn from_anyhow(error: anyhow::Error) -> Self {
        Self {
            messages: error.chain().map(|e| e.to_string()).collect(),
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/anime", get(api_anime_get))
        .route("/kitsu/anime/search", get(api_kitsu_search))
}

async fn api_anime_get(State(_app_state): State<Arc<AppState>>) -> impl IntoResponse {
    Json("WIP")
}

#[derive(Debug, serde::Deserialize)]
struct KitsuSearchParams {
    text: String,
}

#[derive(Debug, serde::Serialize)]
struct ApiKitsuAnime {
    id: u64,
    synopsis: Option<String>,
    title: String,
    rating: Option<String>,
}

async fn api_kitsu_search(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<KitsuSearchParams>,
) -> impl IntoResponse {
    let result = app_state
        .search_kitsu(params.text.as_str())
        .await
        .map(|anime| {
            anime
                .iter()
                .map(|anime| ApiKitsuAnime {
                    id: anime.id,
                    synopsis: anime.synopsis.clone(),
                    title: anime.title.clone(),
                    rating: anime.rating.clone(),
                })
                .collect::<Vec<_>>()
        })
        .map_err(|error| {
            error!("{error:?}");
            ApiError::from_anyhow(error)
        });

    match result {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response(),
    }
}

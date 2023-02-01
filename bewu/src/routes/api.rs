use crate::AppState;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use axum::Router;
use std::num::NonZeroU64;
use std::sync::Arc;
use tracing::error;
use url::Url;

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
        .route("/kitsu/anime", get(api_kitsu_anime))
        .route("/kitsu/anime/:id", get(api_kitsu_anime_id))
        .route(
            "/kitsu/anime/:id/episodes",
            get(api_kitsu_anime_id_episodes),
        )
        .route("/kitsu/episodes/:id", get(api_kitsu_episodes_id))
        .route("/vidstreaming/:id", get(api_vidstreaming_id))
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
    id: NonZeroU64,
    synopsis: Option<String>,
    title: String,
    rating: Option<String>,

    poster_large: String,
}

#[derive(Debug, serde::Serialize)]
struct ApiKitsuEpisode {
    id: NonZeroU64,

    title: Option<String>,

    thumbnail_original: Option<String>,
}

async fn api_kitsu_anime(
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

                    poster_large: anime.poster_large.clone(),
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

async fn api_kitsu_anime_id(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<NonZeroU64>,
) -> impl IntoResponse {
    let result = app_state
        .get_kitsu_anime(id)
        .await
        .map(|anime| ApiKitsuAnime {
            id: anime.id,
            synopsis: anime.synopsis.clone(),
            title: anime.title.clone(),
            rating: anime.rating.clone(),

            poster_large: anime.poster_large.clone(),
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

async fn api_kitsu_anime_id_episodes(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<NonZeroU64>,
) -> impl IntoResponse {
    let result = app_state
        .get_kitsu_anime_episodes(id)
        .await
        .map(|episodes| {
            episodes
                .iter()
                .map(|episode| ApiKitsuEpisode {
                    id: episode.episode_id,
                    title: episode.title.as_ref().map(ToString::to_string),
                    thumbnail_original: episode
                        .thumbnail_original
                        .as_ref()
                        .map(ToString::to_string),
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

async fn api_kitsu_episodes_id(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<NonZeroU64>,
) -> impl IntoResponse {
    let result = app_state
        .get_kitsu_episode(id)
        .await
        .map(|episode| ApiKitsuEpisode {
            id: episode.episode_id,
            title: episode.title.as_ref().map(ToString::to_string),
            thumbnail_original: episode.thumbnail_original.as_ref().map(ToString::to_string),
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

#[derive(Debug, serde::Serialize)]
struct ApiVidstreamingEpisode {
    best_source: Url,
}

async fn api_vidstreaming_id(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<NonZeroU64>,
) -> impl IntoResponse {
    let result = app_state
        .get_vidstreaming_episode(id)
        .await
        .map(|episode| ApiVidstreamingEpisode {
            best_source: episode.best_source,
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

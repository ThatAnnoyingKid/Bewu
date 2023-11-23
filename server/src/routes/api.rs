use crate::app_state::VidstreamingDownloadStateUpdate;
use crate::AppState;
use anyhow::Context;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse;
use axum::response::IntoResponse;
use axum::response::Sse;
use axum::routing::get;
use axum::Json;
use axum::Router;
use bewu_util::StateUpdateItem;
use std::num::NonZeroU64;
use std::sync::Arc;
use tokio_stream::StreamExt;
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

impl<E> From<&E> for ApiError
where
    E: std::error::Error + 'static,
{
    fn from(error: &E) -> Self {
        Self {
            messages: anyhow::Chain::new(error).map(|e| e.to_string()).collect(),
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
        .route(
            "/vidstreaming/:id/download",
            get(api_vidstreaming_id_download),
        )
}

async fn api_anime_get(State(_app_state): State<Arc<AppState>>) -> impl IntoResponse {
    Json("WIP")
}

#[derive(Debug, serde::Deserialize)]
struct KitsuSearchParams {
    text: Option<String>,
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
    let result = async move {
        let text = params.text.context("missing `text` query param")?;
        app_state.search_kitsu(&text).await
    }
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
    url: Option<String>,
}

async fn api_vidstreaming_id(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<NonZeroU64>,
) -> impl IntoResponse {
    let result = app_state
        .get_vidstreaming_episode(id)
        .await
        .map(|episode| ApiVidstreamingEpisode {
            url: episode
                .path
                .map(|path| format!("/data/vidstreaming/sub/{path}")),
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
struct ApiVidstreamingDownloadState {
    #[serde(serialize_with = "serialize_optional_arc_str")]
    info: Option<Arc<str>>,

    progress: f32,
    error: Option<ApiError>,
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "type")]
enum ApiVidstreamingDownloadStateUpdate {
    #[serde(rename = "info")]
    Info {
        #[serde(serialize_with = "serialize_arc_str")]
        info: Arc<str>,
    },
    #[serde(rename = "progress")]
    Progress { progress: f32 },
    #[serde(rename = "error")]
    Error { error: ApiError },
}

fn serialize_optional_arc_str<S>(v: &Option<Arc<str>>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;

    v.as_ref().map(|v| &**v).serialize(s)
}

fn serialize_arc_str<S>(v: &Arc<str>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(v)
}

async fn api_vidstreaming_id_download(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<NonZeroU64>,
) -> impl IntoResponse {
    let result = app_state
        .start_vidstreaming_episode_download(id)
        .await
        .map_err(|error| {
            error!("{error:?}");
            ApiError::from_anyhow(error)
        });

    match result {
        Ok(result) => Sse::new(
            result
                .map(|event| match event {
                    StateUpdateItem::State(state) => {
                        let state = state.get_inner();

                        let state = ApiVidstreamingDownloadState {
                            info: state.info.clone(),
                            progress: state.progress,
                            error: state.error.as_ref().map(ApiError::from),
                        };

                        sse::Event::default().json_data(state)
                    }
                    StateUpdateItem::Update(update) => match update {
                        VidstreamingDownloadStateUpdate::Info { info } => {
                            let update = ApiVidstreamingDownloadStateUpdate::Info { info };
                            sse::Event::default().json_data(update)
                        }
                        VidstreamingDownloadStateUpdate::Progress { progress } => {
                            let update = ApiVidstreamingDownloadStateUpdate::Progress { progress };
                            sse::Event::default().json_data(update)
                        }
                        VidstreamingDownloadStateUpdate::Error { error } => {
                            let update = ApiVidstreamingDownloadStateUpdate::Error {
                                error: ApiError::from(&error),
                            };
                            sse::Event::default().json_data(update)
                        }
                    },
                })
                .chain(tokio_stream::once(
                    sse::Event::default().event("close").json_data("close"),
                )),
        )
        .into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response(),
    }
}

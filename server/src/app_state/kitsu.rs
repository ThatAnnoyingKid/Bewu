use super::Database;
use super::KitsuAnime;
use anyhow::Context;
use bewu_util::AsyncTimedLruCache;
use nd_util::ArcAnyhowError;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::task::JoinSet;
use tracing::error;
use tracing::info;
use tracing::warn;

type SearchCache = AsyncTimedLruCache<Box<str>, SearchResult>;
type SearchResult<E = ArcAnyhowError> = Result<Arc<[KitsuAnime]>, E>;

#[derive(Debug)]
enum KitsuTaskMessage {
    Close {
        tx: tokio::sync::oneshot::Sender<()>,
    },
    Search {
        query: Box<str>,
        tx: tokio::sync::oneshot::Sender<SearchResult<anyhow::Error>>,
    },
    GetAnime {
        id: NonZeroU64,
        tx: tokio::sync::oneshot::Sender<anyhow::Result<Arc<KitsuAnime>>>,
    },
}

#[derive(Debug)]
pub struct KitsuTask {
    tx: tokio::sync::mpsc::Sender<KitsuTaskMessage>,
    handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl KitsuTask {
    pub fn new(database: Database) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        let handle = tokio::spawn(kitsu_task_impl(rx, database));

        Self {
            tx,
            handle: std::sync::Mutex::new(Some(handle)),
        }
    }

    pub async fn search(&self, query: &str) -> SearchResult<anyhow::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(KitsuTaskMessage::Search {
                query: query.into(),
                tx,
            })
            .await?;
        rx.await?
    }

    pub async fn get_anime(&self, id: NonZeroU64) -> anyhow::Result<Arc<KitsuAnime>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(KitsuTaskMessage::GetAnime { id, tx }).await?;
        rx.await?
    }

    async fn close(&self) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(KitsuTaskMessage::Close { tx }).await?;
        rx.await?;
        Ok(())
    }

    async fn join(&self) -> anyhow::Result<()> {
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .context("missing handle")?;

        handle.await?;

        Ok(())
    }

    /// Close and join the task
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        // If we failed to close,
        // its probably because the task is dead.
        // Therefore, it is safe to join.
        let close_result = self.close().await;
        let join_result = self.join().await;

        join_result.or(close_result)
    }
}

async fn kitsu_task_impl(
    mut rx: tokio::sync::mpsc::Receiver<KitsuTaskMessage>,
    database: Database,
) {
    let client = kitsu::Client::new();

    let search_cache = Arc::new(AsyncTimedLruCache::new(128, Duration::from_secs(0)));
    let get_anime_cache = Arc::new(AsyncTimedLruCache::new(128, Duration::from_secs(0)));
    let mut join_set = JoinSet::new();

    loop {
        tokio::select! {
            message = rx.recv() => {
                match message {
                    Some(KitsuTaskMessage::Close { tx }) => {
                        rx.close();
                        let _ = tx.send(()).is_ok();
                    }
                    Some(KitsuTaskMessage::Search { query, tx }) => {
                        let client = client.clone();
                        let search_cache = search_cache.clone();
                        let database = database.clone();
                        join_set.spawn(search_task_impl(
                            client,
                            search_cache,
                            database,
                            query,
                            tx,
                        ));
                    }
                    Some(KitsuTaskMessage::GetAnime { id, tx }) => {
                        let client = client.clone();
                        let get_anime_cache = get_anime_cache.clone();
                        let database = database.clone();
                        join_set.spawn(get_anime_task_impl(
                            client,
                            get_anime_cache,
                            database,
                            id,
                            tx
                        ));
                    }
                    None => {
                        break;
                    }
                }
            }
            Some(result) = join_set.join_next() => {
                match result.context("failed to join task") {
                    Ok(()) => {}
                    Err(error) => {
                        warn!("{error}");
                    }
                }
            }
        }
    }
}

async fn search_task_impl(
    client: kitsu::Client,
    search_cache: Arc<SearchCache>,
    database: Database,
    query: Box<str>,
    tx: tokio::sync::oneshot::Sender<SearchResult<anyhow::Error>>,
) {
    let result = search_cache
        .get(query.clone(), || async move {
            let anime_result = kitsu_search(&client, &query)
                .await
                .map_err(ArcAnyhowError::new);

            if let Ok(anime) = anime_result.as_ref() {
                let anime = anime.clone();
                tokio::spawn(async move {
                    let result = database.upsert_kitsu_anime(anime).await;

                    match result.context("failed to cache search results") {
                        Ok(()) => {}
                        Err(error) => {
                            error!("{error:?}");
                        }
                    }
                });
            }

            anime_result
        })
        .await
        .map_err(anyhow::Error::from);

    let _ = tx.send(result).is_ok();
}

async fn get_anime_task_impl(
    client: kitsu::Client,
    request_map: Arc<AsyncTimedLruCache<NonZeroU64, Result<Arc<KitsuAnime>, ArcAnyhowError>>>,
    database: Database,
    id: NonZeroU64,
    tx: tokio::sync::oneshot::Sender<anyhow::Result<Arc<KitsuAnime>>>,
) {
    let result = request_map
        .get(id, || async move {
            let maybe_anime_result = database
                .get_kitsu_anime(id)
                .await
                .map_err(anyhow::Error::from);

            if let Ok(Some(anime)) = maybe_anime_result {
                return Ok(anime);
            }

            let result = kitsu_get_anime(&client, id)
                .await
                .map_err(ArcAnyhowError::new);

            if let Ok(anime) = result.as_ref() {
                let result = database.upsert_kitsu_anime(anime.clone()).await;

                match result.context("failed to cache search results") {
                    Ok(()) => {}
                    Err(error) => {
                        error!("{error:?}");
                    }
                }
            }

            result
        })
        .await
        .map_err(anyhow::Error::new);

    let _ = tx.send(result).is_ok();
}

//
// Fetch Wrappers
//

async fn kitsu_search(client: &kitsu::Client, query: &str) -> SearchResult<anyhow::Error> {
    info!("searching for \"{query}\"");

    let document = client.search(query).await?;
    let document_data = document.data.context("missing document data")?;

    let mut anime = Vec::with_capacity(document_data.len());
    let last_update = SystemTime::UNIX_EPOCH.elapsed()?.as_secs();
    for item in document_data {
        let attributes = item.attributes.context("missing attributes")?;

        let id: NonZeroU64 = item.id.as_deref().context("missing id")?.parse()?;
        let slug = attributes.slug;
        let synopsis = attributes.synopsis;
        let title = attributes.canonical_title;
        let rating = attributes.average_rating;
        let poster_large = attributes.poster_image.large.into();

        anime.push(KitsuAnime {
            id,
            slug,
            synopsis,
            title,
            rating,
            poster_large,
            last_update,
        });
    }
    let anime: Arc<[KitsuAnime]> = anime.into();

    Ok(anime)
}

async fn kitsu_get_anime(
    client: &kitsu::Client,
    id: NonZeroU64,
) -> anyhow::Result<Arc<KitsuAnime>> {
    info!("getting anime \"{id}\"");

    let document = client.get_anime(id).await?;
    let document_data = document.data.context("missing document data")?;

    let last_update = SystemTime::UNIX_EPOCH.elapsed()?.as_secs();

    let attributes = document_data.attributes.context("missing attributes")?;
    let id: NonZeroU64 = document_data.id.as_deref().context("missing id")?.parse()?;
    let slug = attributes.slug;
    let synopsis = attributes.synopsis;
    let title = attributes.canonical_title;
    let rating = attributes.average_rating;
    let poster_large = attributes.poster_image.large.into();

    let anime = Arc::new(KitsuAnime {
        id,
        slug,
        synopsis,
        title,
        rating,
        poster_large,
        last_update,
    });

    Ok(anime)
}

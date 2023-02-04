mod database;
mod kitsu;
mod vidstreaming;

// Database re-exports
pub use self::database::KitsuAnime;
pub use self::database::KitsuAnimeEpisode;

// Tasks
use self::database::Database;
use self::kitsu::KitsuTask;
use self::vidstreaming::VidstreamingTask;
use crate::util::AsyncLockFile;

pub use self::vidstreaming::CloneDownloadState as VidstreamingDownloadState;
pub use self::vidstreaming::DownloadStateUpdate as VidstreamingDownloadStateUpdate;
use crate::util::AbortJoinHandle;
use anyhow::ensure;
use anyhow::Context;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tokio_stream::Stream;
use tracing::debug;
use url::Url;

#[derive(Debug)]
pub struct VidstreamingEpisode {
    /// The url of the best source
    pub best_source: Url,
}

/// The app state
///
///
/// # Task Structure
/// ```
/// +----------+      +-----------+
/// | Database | ---> | KitsuTask |
/// +----------+      +-----------+
///
/// +---------------+
/// | AsyncLockFile |
/// +---------------+
///
/// ```
pub struct AppState {
    lock_file: AsyncLockFile,
    database: Database,
    kitsu_task: KitsuTask,
    vidstreaming_task: VidstreamingTask,

    kitsu_client: ::kitsu::Client,

    vidstreaming_download: std::sync::Mutex<Option<AbortJoinHandle<()>>>,
}

impl AppState {
    pub async fn new<P>(data_directory: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let data_directory = data_directory.as_ref();
        match tokio::fs::create_dir(&data_directory).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create data directory \"{}\"",
                        data_directory.display()
                    )
                });
            }
        }

        let kitsu_directory = data_directory.join("kistu");
        match tokio::fs::create_dir(&kitsu_directory).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create kitsu directory \"{}\"",
                        kitsu_directory.display()
                    )
                });
            }
        }

        let kitsu_cover_directory = kitsu_directory.join("cover");
        match tokio::fs::create_dir(&kitsu_cover_directory).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create kitsu cover directory \"{}\"",
                        kitsu_cover_directory.display()
                    )
                });
            }
        }

        let kitsu_poster_directory = kitsu_directory.join("poster");
        match tokio::fs::create_dir(&kitsu_poster_directory).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create kitsu poster directory \"{}\"",
                        kitsu_poster_directory.display()
                    )
                });
            }
        }

        let vidstreaming_directory = data_directory.join("vidstreaming");
        match tokio::fs::create_dir(&vidstreaming_directory).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create vidstreaming directory \"{}\"",
                        vidstreaming_directory.display()
                    )
                });
            }
        }

        let vidstreaming_sub_directory = vidstreaming_directory.join("sub");
        match tokio::fs::create_dir(&vidstreaming_sub_directory).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create vidstreaming sub directory \"{}\"",
                        vidstreaming_sub_directory.display()
                    )
                });
            }
        }

        let vidstreaming_dub_directory = vidstreaming_directory.join("dub");
        match tokio::fs::create_dir(&vidstreaming_dub_directory).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create vidstreaming dub directory \"{}\"",
                        vidstreaming_sub_directory.display()
                    )
                });
            }
        }

        let lock_file_path = data_directory.join("bewu.lock");
        let lock_file = AsyncLockFile::create(lock_file_path).await?;
        lock_file
            .try_lock()
            .await
            .context("another process is using the data directory")?;

        let database_path = data_directory.join("database.db");
        let database = Database::new(database_path)
            .await
            .context("failed to open database")?;

        let kitsu_client = ::kitsu::Client::new();

        let kitsu_task = KitsuTask::new(database.clone());
        let vidstreaming_task = VidstreamingTask::new(&vidstreaming_sub_directory);

        Ok(Self {
            lock_file,
            database,
            kitsu_task,
            vidstreaming_task,

            kitsu_client,

            vidstreaming_download: std::sync::Mutex::new(None),
        })
    }

    /// Run a search on kitsu.
    pub async fn search_kitsu(&self, query: &str) -> anyhow::Result<Arc<[KitsuAnime]>> {
        let anime = self.kitsu_task.search(query).await?;
        Ok(anime)
    }

    /// Get the kitsu anime for the given id.
    pub async fn get_kitsu_anime(&self, id: NonZeroU64) -> anyhow::Result<Arc<KitsuAnime>> {
        let anime = self.kitsu_task.get_anime(id).await?;
        Ok(anime)
    }

    /// Get kitsu episodes for the anime with the given id
    pub async fn get_kitsu_anime_episodes(
        &self,
        anime_id: NonZeroU64,
    ) -> anyhow::Result<Arc<[KitsuAnimeEpisode]>> {
        let document = self.kitsu_client.get_anime_episodes(anime_id).await?;
        let document_data = document.data.context("missing document data")?;

        let mut episodes = Vec::with_capacity(document_data.len());
        let last_update = SystemTime::UNIX_EPOCH.elapsed()?.as_secs();
        for item in document_data {
            let attributes = item.attributes.context("missing attributes")?;
            let episode_id: NonZeroU64 = item.id.as_deref().context("missing id")?.parse()?;

            let title = attributes.canonical_title;
            let synopsis = attributes.synopsis;
            let length_minutes: Option<u32> = attributes.length;
            let number = attributes.number;
            let thumbnail_original = attributes
                .thumbnail
                .map(|thumbnail| thumbnail.original.into());

            episodes.push(KitsuAnimeEpisode {
                anime_id,
                episode_id,

                title,
                synopsis,
                length_minutes,
                number,

                thumbnail_original,
                last_update,
            });
        }
        let episodes: Arc<[KitsuAnimeEpisode]> = episodes.into();

        self.database
            .upsert_kitsu_episodes(episodes.clone())
            .await?;

        Ok(episodes)
    }

    /// Get a kitsu episode by id.
    pub async fn get_kitsu_episode(
        &self,
        id: NonZeroU64,
    ) -> anyhow::Result<Arc<KitsuAnimeEpisode>> {
        let document_handle = {
            let client = self.kitsu_client.clone();
            tokio::spawn(async move { client.get_episode(id).await })
        };

        // TODO: Can we avoid a look-up by using the database?
        // Note: I hate that this is necessary.
        let anime_id_handle = {
            // TODO: Investigate url more and add to kitsu lib
            #[derive(Debug, serde::Deserialize)]
            struct EpisodeMediaRelationshipMedia {
                /// The anime id
                pub id: String,

                /// The anime type
                #[serde(rename = "type")]
                pub kind: String,
            }

            let client = self.kitsu_client.client.clone();
            let url = format!("https://kitsu.io/api/edge/episodes/{id}/relationships/media");
            tokio::spawn(async move {
                let document = client
                    .get_json_document::<EpisodeMediaRelationshipMedia>(&url)
                    .await?;
                let document_data = document.data.context("missing document data")?;

                ensure!(document_data.kind == "anime");

                let id: NonZeroU64 = document_data.id.parse()?;

                Result::<_, anyhow::Error>::Ok(id)
            })
        };

        let anime_id = anime_id_handle.await??;

        let document = document_handle.await??;
        let document_data = document.data.context("missing document data")?;

        let last_update = SystemTime::UNIX_EPOCH.elapsed()?.as_secs();

        let attributes = document_data.attributes.context("missing attributes")?;
        let episode_id: NonZeroU64 = document_data.id.as_deref().context("missing id")?.parse()?;

        let title = attributes.canonical_title;
        let synopsis = attributes.synopsis;
        let length_minutes: Option<u32> = attributes.length;
        let number = attributes.number;
        let thumbnail_original = attributes
            .thumbnail
            .map(|thumbnail| thumbnail.original.into());

        let episode = Arc::new(KitsuAnimeEpisode {
            anime_id,
            episode_id,

            title,
            synopsis,
            length_minutes,
            number,

            thumbnail_original,
            last_update,
        });

        self.database.upsert_kitsu_episodes(episode.clone()).await?;

        Ok(episode)
    }

    /// Get a vidstreaming episode.
    pub async fn get_vidstreaming_episode(
        &self,
        id: NonZeroU64,
    ) -> anyhow::Result<
        impl Stream<
            Item = bewu_util::StateUpdateItem<
                VidstreamingDownloadState,
                VidstreamingDownloadStateUpdate,
            >,
        >,
    > {
        let episode = self.get_kitsu_episode(id).await?;
        let anime = self.get_kitsu_anime(episode.anime_id).await?;

        let stream = self
            .vidstreaming_task
            .start_episode_download(anime.slug.as_str(), episode.number)
            .await?;

        Ok(stream)
    }

    /// Shutdown the app state.
    ///
    /// This should only be called once.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let handle = self
            .vidstreaming_download
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(handle) = handle {
            let handle = handle.into_inner();

            handle.abort();

            // TODO: Don't let an error here stop the shutdown sequence
            handle.await?;
        }

        debug!("shutting down vidstreaming task");
        let vidstreaming_shutdown_result = self
            .vidstreaming_task
            .shutdown()
            .await
            .context("failed to shutdwon vidstreaming task");

        debug!("shutting down kitsu task");
        let kitsu_shutdown_result = self
            .kitsu_task
            .shutdown()
            .await
            .context("failed to shutdown kitsu task");

        debug!("shutting down database");
        let database_shutdown_result = self
            .database
            .shutdown()
            .await
            .context("failed to shutdown the database");

        debug!("unlocking lock file");
        let lock_file_unlock_result = self.lock_file.unlock().await;

        debug!("shutting down lock file thread");
        let lock_file_shutdown_result = self
            .lock_file
            .shutdown()
            .await
            .context("failed to shutdown the lock file thread");

        database_shutdown_result
            .or(kitsu_shutdown_result)
            .or(vidstreaming_shutdown_result)
            .or(lock_file_unlock_result)
            .or(lock_file_shutdown_result)
    }
}

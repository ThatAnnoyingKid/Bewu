mod database;

pub use self::database::AnimeEpisode;
pub use self::database::Database;
pub use self::database::KitsuAnime;
use crate::util::AsyncLockFile;
use anyhow::ensure;
use anyhow::Context;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::debug;
use url::Url;

#[derive(Debug)]
pub struct VidstreamingEpisode {
    /// The url of the best source
    pub best_source: Url,
}

pub struct AppState {
    lock_file: AsyncLockFile,
    database: Database,

    kitsu_client: kitsu::Client,
    vidstreaming_client: vidstreaming::Client,
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

        let kitsu_client = kitsu::Client::new();
        let vidstreaming_client = vidstreaming::Client::new();

        Ok(Self {
            lock_file,
            database,

            kitsu_client,
            vidstreaming_client,
        })
    }

    /// Run a search on kitsu.
    ///
    /// All returned data is cached.
    pub async fn search_kitsu(&self, query: &str) -> anyhow::Result<Arc<[KitsuAnime]>> {
        let document = self.kitsu_client.search(query).await?;
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

        self.database.upsert_kitsu_anime(anime.clone()).await?;

        Ok(anime)
    }

    /// Get the kitsu anime for the given id.
    pub async fn get_kitsu_anime(&self, id: NonZeroU64) -> anyhow::Result<KitsuAnime> {
        let document = self.kitsu_client.get_anime(id).await?;
        let document_data = document.data.context("missing document data")?;

        let last_update = SystemTime::UNIX_EPOCH.elapsed()?.as_secs();

        let attributes = document_data.attributes.context("missing attributes")?;
        let id: NonZeroU64 = document_data.id.as_deref().context("missing id")?.parse()?;
        let slug = attributes.slug;
        let synopsis = attributes.synopsis;
        let title = attributes.canonical_title;
        let rating = attributes.average_rating;
        let poster_large = attributes.poster_image.large.into();

        let anime = KitsuAnime {
            id,
            slug,
            synopsis,
            title,
            rating,
            poster_large,
            last_update,
        };

        // TODO: Consider adding special call for single anime.
        self.database
            .upsert_kitsu_anime(Arc::from(std::slice::from_ref(&anime)))
            .await?;

        Ok(anime)
    }

    /// Get kitsu episodes for the given anime id
    pub async fn get_kitsu_episodes(
        &self,
        anime_id: NonZeroU64,
    ) -> anyhow::Result<Arc<[AnimeEpisode]>> {
        let document = self.kitsu_client.get_anime_episodes(anime_id).await?;
        let document_data = document.data.context("missing document data")?;

        let mut episodes = Vec::with_capacity(document_data.len());
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

            episodes.push(AnimeEpisode {
                anime_id,
                episode_id,

                title,
                synopsis,
                length_minutes,
                number,

                thumbnail_original,
            });
        }
        let episodes: Arc<[AnimeEpisode]> = episodes.into();

        self.database
            .update_kitsu_episodes(episodes.clone())
            .await?;

        Ok(episodes)
    }

    /// Get a kitsu episode by id
    pub async fn get_kitsu_episode(&self, id: NonZeroU64) -> anyhow::Result<AnimeEpisode> {
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

        let attributes = document_data.attributes.context("missing attributes")?;
        let episode_id: NonZeroU64 = document_data.id.as_deref().context("missing id")?.parse()?;

        let title = attributes.canonical_title;
        let synopsis = attributes.synopsis;
        let length_minutes: Option<u32> = attributes.length;
        let number = attributes.number;
        let thumbnail_original = attributes
            .thumbnail
            .map(|thumbnail| thumbnail.original.into());

        let episode = AnimeEpisode {
            anime_id,
            episode_id,

            title,
            synopsis,
            length_minutes,
            number,

            thumbnail_original,
        };

        self.database
            .update_kitsu_episodes(Arc::from(std::slice::from_ref(&episode)))
            .await?;

        Ok(episode)
    }

    /// Get a vidstreaming episode.
    pub async fn get_vidstreaming_episode(
        &self,
        id: NonZeroU64,
    ) -> anyhow::Result<VidstreamingEpisode> {
        let episode = self.get_kitsu_episode(id).await?;
        let anime = self.get_kitsu_anime(episode.anime_id).await?;

        // Guess vidstreaming url
        let url = format!(
            "https://gogohd.net/videos/{}-episode-{}",
            anime.slug, episode.number,
        );

        debug!("using vidstreaming url \"{}\"", url);

        let vidstreaming_episode = self.vidstreaming_client.get_episode(url.as_str()).await?;
        let video_player = self
            .vidstreaming_client
            .get_video_player(vidstreaming_episode.video_player_url.as_str())
            .await?;
        let video_data = self
            .vidstreaming_client
            .get_video_player_video_data(&video_player)
            .await?;

        debug!("located {} sources", video_data.source.len());
        for source in video_data.source.iter() {
            debug!(
                "found source: (url={}, label={}, kind={})",
                source.file, source.label, source.kind
            );
        }

        // debug!("{:#?}", video_data.source_bk);

        let best_source = video_data
            .get_best_source()
            .context("failed to select source")?;
        debug!(
            "selected source: (url={}, label={}, kind={})",
            best_source.file, best_source.label, best_source.kind
        );

        Ok(VidstreamingEpisode {
            best_source: best_source.file.clone(),
        })
    }

    /// Shutdown the app state.
    ///
    /// This should only be called once.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
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
            .or(lock_file_unlock_result)
            .or(lock_file_shutdown_result)
    }
}

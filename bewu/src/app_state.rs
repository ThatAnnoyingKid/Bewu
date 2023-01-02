mod database;

pub use self::database::Anime;
pub use self::database::Database;
use crate::util::AsyncLockFile;
use anyhow::Context;
use std::path::Path;
use std::sync::Arc;

pub struct AppState {
    lock_file: AsyncLockFile,
    database: Database,
    kitsu_client: kitsu::Client,
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

        Ok(Self {
            lock_file,
            database,
            kitsu_client,
        })
    }

    /// Run a search on kitsu
    ///
    /// All returned data is cached.
    pub async fn search_kitsu(&self, query: &str) -> anyhow::Result<Arc<[Anime]>> {
        let document = self.kitsu_client.search(query).await?;
        let document_data = document.data.context("missing document data")?;

        let mut anime = Vec::with_capacity(document_data.len());
        for item in document_data {
            let attributes = item.attributes.context("missing attributes")?;

            let id: u64 = item.id.as_deref().context("missing id")?.parse()?;
            let slug = attributes.slug;
            let synopsis = attributes.synopsis;
            let title = attributes.canonical_title;
            let rating = attributes.average_rating;

            anime.push(Anime {
                id,
                slug,
                synopsis,
                title,
                rating,
            });
        }
        let anime: Arc<[Anime]> = anime.into();

        self.database.update_kitsu_anime(anime.clone()).await?;

        Ok(anime)
    }

    /// Shutdown the app state.
    ///
    /// This should only be called once
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let lock_file_unlock_result = self.lock_file.unlock().await;
        let lock_file_shutdown_result = self
            .lock_file
            .shutdown()
            .await
            .context("failed to shutdown the lock file thread");

        let database_shutdown_result = self
            .database
            .shutdown()
            .await
            .context("failed to shutdown the database");

        database_shutdown_result
            .or(lock_file_unlock_result)
            .or(lock_file_shutdown_result)
    }
}

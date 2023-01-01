use crate::util::AsyncLockFile;
use anyhow::Context;
use std::path::Path;

#[derive(Debug)]
pub struct Database {
    database: async_rusqlite::Database,
}

impl Database {
    pub async fn new<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let database = async_rusqlite::Database::open(path, true, |_database| Ok(())).await?;
        Ok(Self { database })
    }

    /// Shut down the database.
    ///
    /// Should only be called once.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.database
            .close()
            .await
            .context("failed to send close command")?;
        self.database
            .join()
            .await
            .context("failed to join database thread")
    }
}

pub struct AppState {
    lock_file: AsyncLockFile,
    database: Database,
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

        Ok(Self {
            lock_file,
            database,
        })
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

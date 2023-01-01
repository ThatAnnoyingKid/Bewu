use crate::util::AsyncLockFile;
use anyhow::Context;
use std::path::Path;
use tracing::error;

const SETUP_SQL: &str = include_str!("../sql/setup.sql");

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
        let database = async_rusqlite::Database::open(path, true, |database| {
            database
                .execute_batch(SETUP_SQL)
                .context("failed to setup database")?;
            Ok(())
        })
        .await?;
        Ok(Self { database })
    }

    /// Shut down the database.
    ///
    /// Should only be called once.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let optmize_result = self
            .database
            .access_db(|database| {
                database.execute("PRAGMA OPTIMIZE;", [])?;
                database.execute("VACUUM;", [])
            })
            .await
            .context("failed to access database")
            .and_then(|v| v.context("failed to execute shutdown commands"))
            .map(|_| ());

        if let Err(e) = optmize_result.as_ref() {
            error!("{}", e);
        }

        self.database
            .close()
            .await
            .context("failed to send close command")?;
        let join_result = self
            .database
            .join()
            .await
            .context("failed to join database thread");
        join_result.or(optmize_result)
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

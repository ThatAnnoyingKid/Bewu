use crate::util::AsyncLockFile;
use anyhow::Context;
use std::path::Path;

#[derive(Debug)]
pub struct Database {}

pub struct AppState {
    lock_file: AsyncLockFile,
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

        // let database_path = data_directory.join("database.db");

        Ok(Self { lock_file })
    }

    /// Shutdown the app state.
    ///
    /// This should only be called once
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let unlock_result = self.lock_file.unlock().await;
        let shutdown_result = self
            .lock_file
            .shutdown()
            .await
            .context("failed to shutdown the lock file thread");

        unlock_result.or(shutdown_result)
    }
}

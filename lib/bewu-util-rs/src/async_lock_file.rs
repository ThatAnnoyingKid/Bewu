use anyhow::anyhow;
use anyhow::Context;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
enum Message {
    Close {
        tx: tokio::sync::oneshot::Sender<()>,
    },
    Lock {
        block: bool,
        tx: tokio::sync::oneshot::Sender<anyhow::Result<()>>,
    },
    Unlock {
        tx: tokio::sync::oneshot::Sender<anyhow::Result<()>>,
    },
}

#[derive(Debug)]
pub struct AsyncLockFile {
    handle: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
    tx: tokio::sync::mpsc::Sender<Message>,
}

impl AsyncLockFile {
    /// Open a lock file.
    pub async fn create<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref().to_owned();
        let (open_tx, open_rx) = tokio::sync::oneshot::channel();
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let handle = std::thread::spawn(move || async_lock_file_thread_impl(path, open_tx, rx));

        open_rx
            .await
            .context("lock file thread failed to respond")??;

        Ok(Self {
            handle: std::sync::Mutex::new(Some(handle)),
            tx,
        })
    }

    async fn close(&self) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(Message::Close { tx }).await?;
        rx.await.context("failed to get response")
    }

    /// Send the shutdown signal and wait for the thread to close.
    ///
    /// If this is not called, dropping all the handles will tell the background thread to exit.
    /// This should only be called once.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.close()
            .await
            .context("error sending close message to lock thread")?;

        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .context("missing thread handle")?;
        tokio::task::spawn_blocking(move || handle.join().ok().context("lock thread panicked"))
            .await?
    }

    async fn send_lock_msg(&self, block: bool) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(Message::Lock { tx, block }).await?;
        rx.await
            .context("failed to get response")?
            .context("failed to lock lock file")
    }

    /// Try to lock the file, waiting if it is locked.
    pub async fn lock(&self) -> anyhow::Result<()> {
        self.send_lock_msg(true).await
    }

    /// Lock the file, exiting immediately if it is locked.
    pub async fn try_lock(&self) -> anyhow::Result<()> {
        self.send_lock_msg(false).await
    }

    /// Unlock the file
    pub async fn unlock(&self) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(Message::Unlock { tx }).await?;
        rx.await
            .context("failed to get response")?
            .context("failed to unlock lock file")
    }
}

fn async_lock_file_thread_impl(
    path: PathBuf,
    open_tx: tokio::sync::oneshot::Sender<std::io::Result<()>>,
    mut rx: tokio::sync::mpsc::Receiver<Message>,
) {
    let file = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(path);

    let mut file = match file {
        Ok(file) => {
            if open_tx.send(Ok(())).is_err() {
                return;
            }

            fd_lock::RwLock::new(file)
        }
        Err(e) => {
            let _ = open_tx.send(Err(e)).is_ok();
            return;
        }
    };

    while let Some(msg) = rx.blocking_recv() {
        match msg {
            Message::Close { tx } => {
                rx.close();
                let _ = tx.send(()).is_ok();
            }
            Message::Lock { block, tx } => {
                let result = if block {
                    file.write()
                } else {
                    file.try_write()
                };

                match result {
                    Ok(guard) => {
                        let _ = tx.send(Ok(())).is_ok();

                        while let Some(msg) = rx.blocking_recv() {
                            match msg {
                                Message::Close { tx } => {
                                    rx.close();
                                    let _ = tx.send(()).is_ok();
                                }
                                Message::Lock { tx, .. } => {
                                    let _ = tx
                                        .send(Err(anyhow!("the lock file has already been locked")))
                                        .is_ok();
                                }
                                Message::Unlock { tx } => {
                                    drop(guard);
                                    let _ = tx.send(Ok(())).is_ok();
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into())).is_ok();
                    }
                }
            }
            Message::Unlock { tx } => {
                let _ = tx
                    .send(Err(anyhow!("the lock has already been unlocked")))
                    .is_ok();
            }
        }
    }
}

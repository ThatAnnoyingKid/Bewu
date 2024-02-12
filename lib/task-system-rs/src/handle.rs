use crate::Error;
use crate::TaskMessage;
use std::sync::Arc;
use std::sync::Weak;

/// A handle to a task.
///
/// This can be used to send messages to a task.
#[derive(Debug)]
pub struct TaskHandle<M> {
    inner: Arc<InnerTaskHandle<M>>,
}

impl<M> TaskHandle<M> {
    /// Create a new task handle from its parts.
    pub(crate) fn new(
        tx: tokio::sync::mpsc::Sender<TaskMessage<M>>,
        handle: tokio::task::JoinHandle<()>,
    ) -> Self {
        Self {
            inner: Arc::new(InnerTaskHandle {
                tx,
                handle: std::sync::Mutex::new(Some(handle)),
            }),
        }
    }

    /// Send a task message
    async fn send_task_message(&self, message: TaskMessage<M>) -> Result<(), Error> {
        self.inner
            .tx
            .send(message)
            .await
            .map_err(|_error| Error::TaskClosed)
    }

    /// Send a user message.
    pub async fn send(&self, message: M) -> Result<(), Error> {
        self.send_task_message(TaskMessage::User(message)).await
    }

    /// Send a close message to the task.
    ///
    /// # Returns
    /// Returns when the message has been processed.
    /// Note that the task may not be closed yet, as it may be cleaning up.
    pub async fn close(&self) -> Result<(), Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send_task_message(TaskMessage::Close { tx }).await?;
        rx.await.map_err(|_error| Error::NoResponse)?;
        Ok(())
    }

    /// Join the task.
    ///
    /// This may only be attempted once.
    /// Future calls will return an error.
    pub async fn join(&self) -> Result<(), Error> {
        let handle = self
            .inner
            .handle
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .ok_or(Error::MissingHandle)?;

        handle.await?;

        Ok(())
    }

    /// Close and joins the task.
    ///
    /// This may only be attempted once.
    /// Future calls will return an error.
    /// This will still function even if another task sends a close message,
    /// or if the task itself crashed.
    pub async fn close_and_join(&self) -> Result<(), Error> {
        let _ = self.close().await.is_ok();
        self.join().await
    }

    /// Downgrade this task handle.
    pub fn downgrade(&self) -> WeakTaskHandle<M> {
        WeakTaskHandle {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

impl<M> Clone for TaskHandle<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// The inner handle state
#[derive(Debug)]
pub struct InnerTaskHandle<M> {
    handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    tx: tokio::sync::mpsc::Sender<TaskMessage<M>>,
}

/// A Weak Task Handle
#[derive(Debug)]
pub struct WeakTaskHandle<M> {
    inner: Weak<InnerTaskHandle<M>>,
}

impl<M> WeakTaskHandle<M> {
    /// Upgrade the handle to begin using it.
    pub fn upgrade(&self) -> Option<TaskHandle<M>> {
        Some(TaskHandle {
            inner: self.inner.upgrade()?,
        })
    }
}

impl<M> Clone for WeakTaskHandle<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

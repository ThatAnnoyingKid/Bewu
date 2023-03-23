pub use bewu_util::AsyncLockFile;

/// A join handle wrapper that will abort the task when dropped.
pub struct AbortJoinHandle<T> {
    handle: Option<tokio::task::JoinHandle<T>>,
}

impl<T> AbortJoinHandle<T> {
    /// Wrap a join handle
    pub fn new(handle: tokio::task::JoinHandle<T>) -> Self {
        Self {
            handle: Some(handle),
        }
    }

    /// Get the inner handle, without aborting it.
    pub fn into_inner(mut self) -> tokio::task::JoinHandle<T> {
        self.handle.take().unwrap()
    }
}

impl<T> AsRef<tokio::task::JoinHandle<T>> for AbortJoinHandle<T> {
    fn as_ref(&self) -> &tokio::task::JoinHandle<T> {
        self.handle.as_ref().unwrap()
    }
}

impl<T> Drop for AbortJoinHandle<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.as_ref() {
            handle.abort();
        }
    }
}

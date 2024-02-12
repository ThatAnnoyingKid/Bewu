use super::WeakTaskHandle;
use std::future::Future;
use tokio::task::JoinSet;

/// A context for extra task utilities, like spawning futures scoped to the current task.
pub struct TaskContext<M> {
    pub(crate) join_set: JoinSet<()>,
    pub(crate) task_handle: WeakTaskHandle<M>,
}

impl<M> TaskContext<M> {
    /// Create a new task context.
    pub(crate) fn new(task_handle: WeakTaskHandle<M>) -> Self {
        Self {
            join_set: JoinSet::new(),
            task_handle,
        }
    }

    /// Spawn a future, scoped to the current task.
    pub fn spawn<F>(&mut self, future: F) -> tokio::task::AbortHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.join_set.spawn(future)
    }

    /// Get a weak task handle.
    pub fn task_handle(&self) -> &WeakTaskHandle<M> {
        &self.task_handle
    }
}

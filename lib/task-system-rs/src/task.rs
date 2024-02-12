use crate::TaskContext;
use tokio::task::JoinError;

/// A user defined task
pub trait Task: Send + 'static {
    /// A user-defined message for this task.
    type Message: Send + 'static;

    /// Process user-defined messages.
    fn process_message(
        &mut self,
        _message: Self::Message,
        _task_context: &mut TaskContext<Self::Message>,
    ) {
    }

    /// A close request occured, clean up and shutdown.
    ///
    /// No new messages will be sent to this task.
    fn process_close(&mut self, _task_context: &mut TaskContext<Self::Message>) {}

    /// A scoped future joined.
    fn process_join_result(
        &mut self,
        _result: Result<(), JoinError>,
        _task_context: &mut TaskContext<Self::Message>,
    ) {
    }
}

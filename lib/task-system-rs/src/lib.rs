mod context;
mod handle;
mod task;

pub use self::context::TaskContext;
pub use self::handle::TaskHandle;
pub use self::handle::WeakTaskHandle;
pub use self::task::Task;

/*
Something less full-blown than a full actor system, more modular.
Task -> TaskSystem.

Tasks spawn on the task system.
The task system will join all tasks and handle errors that may occur.
The Task struct is given a message to process.

Task handles should be typed by message.
TaskHandle<UserMessage>.
These handles can then be passed as the constructor for tasks.
let taskhandle = TaskSystem.spawn(TaskHandle);
let newtaskobj = UserTask::new(taskhandle);
let handle = TaskSystem.spawn(newtaskobj);

type safety.

How to bringdown?
reverse order? internal graph? task cycles?
Idea: Punt to user.
Send shutdown system message to all.
Programmer must close a critical task at that point, so shutdown can occur.
Handle shutdown by just allowing to close tasks via handles? await the system?
system should abort all on drop. losing the manager is not acceptable.
*/

/// Library error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the task is closed")]
    TaskClosed,

    #[error("the task did not reply to the message")]
    NoResponse,

    #[error("the task is missing its internal handle")]
    MissingHandle,

    #[error(transparent)]
    TokioJoin(#[from] tokio::task::JoinError),
}

/// Internal message enum of all messages a task can accept.
enum TaskMessage<M> {
    /// A close message
    Close {
        tx: tokio::sync::oneshot::Sender<()>,
    },

    /// A user-defined message
    User(M),
}

/// Spawn a task with the given channel capacity.
pub fn spawn<T>(mut task: T, capacity: usize) -> TaskHandle<T::Message>
where
    T: Task,
{
    let (tx, mut rx) = tokio::sync::mpsc::channel(capacity);
    let (weak_task_handle_tx, weak_task_handle_rx) = tokio::sync::oneshot::channel();

    let handle = tokio::spawn(async move {
        let weak_task_handle = match weak_task_handle_rx.await {
            Ok(weak_task_handle) => weak_task_handle,
            Err(_error) => {
                return;
            }
        };

        let mut context = TaskContext::new(weak_task_handle);

        loop {
            tokio::select! {
                Some(result) = context.join_set.join_next() => {
                    task.process_join_result(result, &mut context);
                }
                message = rx.recv() => {
                    match message {
                        Some(TaskMessage::Close { tx }) => {
                            rx.close();
                            task.process_close(&mut context);
                            let _ = tx.send(()).is_ok();
                        }
                        Some(TaskMessage::User(message)) => {
                            task.process_message(message, &mut context);
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }

        while let Some(result) = context.join_set.join_next().await {
            task.process_join_result(result, &mut context);
        }
    });

    let task_handle = TaskHandle::new(tx, handle);

    // This should never fail.
    // We make it an option to avoid requiring Debug for Message.
    // This may be fixed with a custom Debug impl for WeakTaskHandle and/or TaskHandle.
    weak_task_handle_tx
        .send(task_handle.downgrade())
        .ok()
        .unwrap();

    task_handle
}

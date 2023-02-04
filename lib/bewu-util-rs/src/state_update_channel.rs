use std::task::ready;
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::Stream;
use tokio_util::sync::ReusableBoxFuture;

/// Either the state if the channel lagged, or an update.
#[derive(Debug)]
pub enum StateUpdateItem<S, U> {
    /// The channel lagged.
    State(S),

    /// The update.
    Update(U),
}

/// Make a bounded channel.
pub fn state_update_channel<S>(
    capacity: usize,
    state: S,
) -> (StateUpdateTx<S, S::Update>, StateUpdateRx<S, S::Update>)
where
    S: State,
{
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);

    (
        StateUpdateTx {
            state: state.clone(),
            stream: tx,
        },
        StateUpdateRx {
            state,
            stream: rx,
            cloned: false,
        },
    )
}

/// A state that is changed via updates.
pub trait State: Clone {
    /// The update that can be applied to this state.
    type Update: Clone;

    // TODO: Should this return an error if the update failed?
    // If an error occurs while updating, this should be encoded into the state somehow?
    //
    /// Apply an update to this state.
    ///
    /// This function should be resilient to duplicate updates.
    fn apply_update(&self, update: &Self::Update);
}

/// The sender for state updates.
#[derive(Debug, Clone)]
pub struct StateUpdateTx<S, U> {
    state: S,
    stream: tokio::sync::broadcast::Sender<U>,
}

impl<S, U> StateUpdateTx<S, U>
where
    S: State<Update = U>,
{
    /// Send an update and apply it to the state.
    pub fn send(&self, update: U) {
        self.state.apply_update(&update);

        // TODO: How to handle no receivers?
        let _ = self.stream.send(update).is_ok();
    }
}

/// The receiver for state updates
#[derive(Debug)]
pub struct StateUpdateRx<S, U> {
    state: S,
    stream: tokio::sync::broadcast::Receiver<U>,

    cloned: bool,
}

impl<S, U> StateUpdateRx<S, U>
where
    S: Clone,
    U: Clone,
{
    /*
    /// Get a reference to the state.
    pub fn state_ref(&self) -> &S {
        &self.state
    }
    */

    /// Get the next update in this stream, or the state if lagging occured.
    ///
    /// If an update occurs, the state should already be updated.
    pub async fn recv(&mut self) -> Option<StateUpdateItem<S, U>> {
        // If this rx handle was cloned, we might have lost messages.
        // Re-send the state and clear the flag.
        if self.cloned {
            self.cloned = false;
            return Some(StateUpdateItem::State(self.state.clone()));
        }

        let result = self.stream.recv().await;
        match result {
            Ok(update) => Some(StateUpdateItem::Update(update)),
            Err(RecvError::Lagged(_n)) => Some(StateUpdateItem::State(self.state.clone())),
            Err(RecvError::Closed) => None,
        }
    }
}

impl<S, U> StateUpdateRx<S, U>
where
    S: Clone + Send + 'static,
    U: Clone + Send + 'static,
{
    pub fn into_stream(self) -> StateUpdateStream<S, U> {
        StateUpdateStream {
            future: ReusableBoxFuture::new(make_stream_future(self)),
        }
    }
}

impl<S, U> Clone for StateUpdateRx<S, U>
where
    S: Clone,
    U: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            stream: self.stream.resubscribe(),
            cloned: true,
        }
    }
}

async fn make_stream_future<S, U>(
    mut rx: StateUpdateRx<S, U>,
) -> (Option<StateUpdateItem<S, U>>, StateUpdateRx<S, U>)
where
    S: Clone,
    U: Clone,
{
    let item = rx.recv().await;
    (item, rx)
}

type StreamFutureOutput<S, U> = (Option<StateUpdateItem<S, U>>, StateUpdateRx<S, U>);

/// A Stream wrapper for a state update receiver
#[derive(Debug)]
pub struct StateUpdateStream<S, U> {
    future: ReusableBoxFuture<'static, StreamFutureOutput<S, U>>,
}

impl<S, U> Stream for StateUpdateStream<S, U>
where
    S: State<Update = U> + Send + 'static,
    U: Clone + Send + 'static,
{
    type Item = StateUpdateItem<S, U>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let (result, rx) = ready!(self.future.poll(cx));
        self.future.set(make_stream_future(rx));
        std::task::Poll::Ready(result)
    }
}

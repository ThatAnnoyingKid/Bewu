use std::task::ready;
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::Stream;
use tokio_util::sync::ReusableBoxFuture;

/// Either the state if the channel lagged, or an update.
#[derive(Debug)]
pub enum StateUpdateItem<S>
where
    S: StateUpdateChannelState,
{
    /// The channel lagged.
    State(S),

    /// The update.
    Update(S::Update),
}

/// Make a bounded channel.
pub fn state_update_channel<S>(capacity: usize, state: S) -> (StateUpdateTx<S>, StateUpdateRx<S>)
where
    S: StateUpdateChannelState,
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
pub trait StateUpdateChannelState: Clone {
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
pub struct StateUpdateTx<S>
where
    S: StateUpdateChannelState,
{
    state: S,
    stream: tokio::sync::broadcast::Sender<S::Update>,
}

impl<S> StateUpdateTx<S>
where
    S: StateUpdateChannelState,
{
    /// Send an update and apply it to the state.
    pub fn send(&self, update: impl Into<S::Update>) {
        let update = update.into();

        self.state.apply_update(&update);

        // TODO: How to handle no receivers?
        let _ = self.stream.send(update).is_ok();
    }

    /// Get an immutable reference to the state.
    ///
    /// Use of this method is discouraged,
    /// as state updates should typically be done through sending updates.
    pub fn state_ref(&self) -> &S {
        &self.state
    }
}

/// The receiver for state updates
#[derive(Debug)]
pub struct StateUpdateRx<S>
where
    S: StateUpdateChannelState,
{
    state: S,
    stream: tokio::sync::broadcast::Receiver<S::Update>,

    cloned: bool,
}

impl<S> StateUpdateRx<S>
where
    S: StateUpdateChannelState,
{
    /// Get an immutable reference to the state.
    ///
    /// Use of this method is discouraged,
    /// as state inspection should typically be done through listening on the channel.
    pub fn state_ref(&self) -> &S {
        &self.state
    }

    /// Get the next update in this stream, or the state if lagging occured.
    ///
    /// If an update occurs, the state should already be updated.
    pub async fn recv(&mut self) -> Option<StateUpdateItem<S>> {
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

impl<S> StateUpdateRx<S>
where
    S: StateUpdateChannelState + Send + 'static,
    S::Update: Send,
{
    pub fn into_stream(self) -> StateUpdateStream<S> {
        StateUpdateStream {
            future: ReusableBoxFuture::new(make_stream_future(self)),
        }
    }
}

impl<S> Clone for StateUpdateRx<S>
where
    S: StateUpdateChannelState,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            stream: self.stream.resubscribe(),
            cloned: true,
        }
    }
}

async fn make_stream_future<S>(
    mut rx: StateUpdateRx<S>,
) -> (Option<StateUpdateItem<S>>, StateUpdateRx<S>)
where
    S: StateUpdateChannelState,
{
    let item = rx.recv().await;
    (item, rx)
}

type StreamFutureOutput<S> = (Option<StateUpdateItem<S>>, StateUpdateRx<S>);

/// A Stream wrapper for a state update receiver
#[derive(Debug)]
pub struct StateUpdateStream<S>
where
    S: StateUpdateChannelState,
{
    future: ReusableBoxFuture<'static, StreamFutureOutput<S>>,
}

impl<S> Stream for StateUpdateStream<S>
where
    S: StateUpdateChannelState + Send + 'static,
    S::Update: Send,
{
    type Item = StateUpdateItem<S>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let (result, rx) = ready!(self.future.poll(cx));
        self.future.set(make_stream_future(rx));
        std::task::Poll::Ready(result)
    }
}

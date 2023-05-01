use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

/// A cache with a single entry, whose value is valid only for a set period of time.
///
/// The value of this cache must impl Clone as it is shared when racing and for the entire valid_for Duration.
/// The easiest way to ensure this is with an `Arc`.
#[derive(Debug)]
pub struct AsyncTimedCacheCell<V> {
    valid_for: Duration,
    value: std::sync::Mutex<Arc<tokio::sync::OnceCell<Entry<V>>>>,
}

impl<V> AsyncTimedCacheCell<V>
where
    V: Clone,
{
    /// Create a new [`AsyncTimedCacheCell`].
    pub fn new(valid_for: Duration) -> Self {
        Self {
            valid_for,
            value: std::sync::Mutex::new(Default::default()),
        }
    }

    /// Get the value in this cell.
    ///
    /// If the value is missing or expired, `func` is called to calculate it.
    /// `func` returns a future which returns a value.
    /// It is guaranteed that func is only called once if get is called in parallel.
    pub async fn get<FN, FUT>(&self, func: FN) -> V
    where
        FN: FnOnce() -> FUT,
        FUT: Future<Output = V>,
    {
        let value = {
            let mut value = self.value.lock().expect("cache poisoned");
            if let Some(entry) = value.get() {
                if entry.created.elapsed() > self.valid_for {
                    std::mem::take(&mut *value);
                } else {
                    return entry.value.clone();
                }
            }

            value.clone()
        };

        value
            .get_or_init(|| async move {
                let value = func().await;
                let created = Instant::now();

                Entry { created, value }
            })
            .await
            .value
            .clone()
    }
}

/// The cache entry
#[derive(Debug)]
struct Entry<V> {
    /// The time this was created at
    created: Instant,

    /// The cache value
    value: V,
}

use lru::LruCache;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

/// An async LRU whose entries are valid for only a certian time period.
///
/// Entries must impl `Clone` as they are shared when racing and for the `valid_for` Duration.
/// The simplest way to do this for a non-Clone type is an `Arc`.
pub struct AsyncTimedLruCache<K, V> {
    cache: std::sync::Mutex<LruCache<K, Arc<tokio::sync::OnceCell<Entry<V>>>>>,
    valid_for: Duration,
}

impl<K, V> AsyncTimedLruCache<K, V>
where
    K: Eq + std::hash::Hash,
    V: Clone,
{
    /// Make a new [`AsyncTimedLruCache`].
    ///
    /// # Panics
    ///
    /// Panics if the capacity is 0.
    /// To simulate a 0-sized cache, instead use 0 for `valid_for` and a `capacity` of 1.
    pub fn new(capacity: usize, valid_for: Duration) -> Self {
        let capacity = capacity.try_into().expect("capacity is 0");
        let cache = std::sync::Mutex::new(LruCache::new(capacity));
        Self { cache, valid_for }
    }

    /// Get an entry by a value, providing a function that returns a future that is called if the cache entry is expired or empty.
    ///
    /// It is guaranteed that the function will be called only once if many gets run in parallel.
    pub async fn get<FN, FUT>(&self, key: K, func: FN) -> V
    where
        FN: FnOnce() -> FUT,
        FUT: Future<Output = V>,
    {
        let entry = {
            let mut cache = self.cache.lock().expect("cache poisoned");

            match cache.get_mut(&key) {
                Some(entry) => match entry.get() {
                    Some(cell_value) => {
                        if cell_value.created.elapsed() > self.valid_for {
                            std::mem::take(entry);
                            entry.clone()
                        } else {
                            return cell_value.value.clone();
                        }
                    }
                    None => {
                        let entry = Arc::new(tokio::sync::OnceCell::new());
                        cache.put(key, entry.clone());
                        entry
                    }
                },
                None => {
                    let entry = Arc::new(tokio::sync::OnceCell::new());
                    cache.put(key, entry.clone());
                    entry
                }
            }
        };

        entry
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

impl<K, V> std::fmt::Debug for AsyncTimedLruCache<K, V>
where
    K: std::hash::Hash + std::cmp::Eq,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncTimedLruCache")
            .field("cache", &self.cache)
            .field("valid_for", &self.valid_for)
            .finish()
    }
}

/// A cache entry
#[derive(Debug)]
struct Entry<V> {
    /// The time this entry was created.
    created: Instant,

    /// The value of the entry.
    value: V,
}

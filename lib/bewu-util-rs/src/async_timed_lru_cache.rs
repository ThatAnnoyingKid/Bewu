use lru::LruCache;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

/// An async LRU whose entries are valid for only a certian time period.
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

    pub async fn get<FN, FUT>(&self, key: K, func: FN) -> V
    where
        FN: FnOnce() -> FUT,
        FUT: Future<Output = V>,
    {
        let entry = {
            let mut cache = self.cache.lock().unwrap();

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

impl<K, V> std::fmt::Debug for AsyncTimedLruCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Provide more info
        f.debug_struct("AsyncTimedLruCache").finish()
    }
}

#[derive(Debug)]
struct Entry<V> {
    created: Instant,
    value: V,
}

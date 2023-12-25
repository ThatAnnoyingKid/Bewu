use std::collections::hash_map::Entry as HashMapEntry;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

/// An error that signifies that someone already holds the lock.
#[derive(Debug)]
pub struct AsyncMutexMapLockError(());

impl std::fmt::Display for AsyncMutexMapLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "the resource is locked".fmt(f)
    }
}

impl std::error::Error for AsyncMutexMapLockError {}

/// A type to regulate async access to resources keyed by a type K.
#[derive(Debug)]
pub struct AsyncMutexMap<K> {
    map: Arc<Mutex<HashMap<K, Arc<Semaphore>>>>,
}

impl<K> AsyncMutexMap<K> {
    /// Create a new [`AsyncMutexMap`].
    pub fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<K> AsyncMutexMap<K>
where
    K: Hash + Eq + Clone,
{
    /// Lock the resource with a given key.
    pub async fn lock(&self, key: K) -> AsyncMutexMapGuard<K> {
        let (mut guard, semaphore) = match self.try_lock_inner(key) {
            Ok(guard) => {
                return guard;
            }
            Err((guard, semaphore)) => (guard, semaphore),
        };

        // Wait for a permit.
        // This signifies a release of ownership.
        // When we get a permit, we now own the resource.
        //
        // We never close the semaphore,
        // so we can unwrap.
        let permit = semaphore.acquire_owned().await.unwrap();

        guard.permit = Some(permit);

        guard
    }

    /// Try to lock the resource with a given key.
    pub fn try_lock(&self, key: K) -> Result<AsyncMutexMapGuard<K>, AsyncMutexMapLockError> {
        let (mut guard, semaphore) = match self.try_lock_inner(key) {
            Ok(guard) => {
                return Ok(guard);
            }
            Err((guard, semaphore)) => (guard, semaphore),
        };

        // Try to get a permit.
        // This signifies a release of ownership.
        // When we get a permit, we now own the resource.
        //
        // We never close the semaphore,
        // so an error here means someone already holds the lock.
        let permit = semaphore
            .try_acquire_owned()
            .map_err(|_error| AsyncMutexMapLockError(()))?;

        guard.permit = Some(permit);

        Ok(guard)
    }

    /// Internal helper function for resource locking
    fn try_lock_inner(
        &self,
        key: K,
    ) -> Result<AsyncMutexMapGuard<K>, (AsyncMutexMapGuard<K>, Arc<Semaphore>)> {
        // We want poisioning to panic, for extra safety.
        let mut map = self.map.lock().unwrap();

        match map.entry(key.clone()) {
            HashMapEntry::Occupied(entry) => {
                // Somebody else has locked this entry.
                // Clone the entry to avoid holding the map lock.
                Err((
                    AsyncMutexMapGuard {
                        map: self.clone(),

                        permit: None,
                        key: Some(key),
                    },
                    entry.get().clone(),
                ))
            }
            HashMapEntry::Vacant(entry) => {
                // Since nobody has locked this entry,
                // insert an entry and claim ownership over it.

                // Nobody else has access to this semaphore as well,
                // so we can unwrap as we know it has 1 permit.
                //
                // We just created the semaphore,
                // so we unwrap since it can't be closed yet.
                let permit = entry
                    .insert(Arc::new(Semaphore::new(1)))
                    .clone()
                    .try_acquire_owned()
                    .unwrap();

                Ok(AsyncMutexMapGuard {
                    map: self.clone(),

                    permit: Some(permit),
                    key: Some(key),
                })
            }
        }
    }
}

impl<K> Default for AsyncMutexMap<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> Clone for AsyncMutexMap<K> {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}

/// A guard representing ownership of a resource with a given key.
#[derive(Debug)]
pub struct AsyncMutexMapGuard<K>
where
    K: Hash + Eq + Clone,
{
    map: AsyncMutexMap<K>,

    permit: Option<OwnedSemaphorePermit>,
    key: Option<K>,
}

impl<K> Drop for AsyncMutexMapGuard<K>
where
    K: Hash + Eq + Clone,
{
    fn drop(&mut self) {
        // Extract permit, if it exists.
        let permit = self.permit.take();

        // Extract key.
        //
        // This will only be extracted on drop.
        // If this is missing, this is a bug.
        // Therefore, we unwrap.
        let key = self.key.take().unwrap();

        // We want poisioning to panic, for extra safety.
        let mut map = self.map.map.lock().unwrap();

        let mut entry = match map.entry(key) {
            HashMapEntry::Occupied(entry) => entry,
            HashMapEntry::Vacant(_entry) => {
                // This should not be possible.
                // The map is corrupted.
                //
                // This is a critical failure,
                // the map has failed to keep track of accesses.
                unreachable!("missing key in map");
            }
        };

        // Drop the permit, to ensure our Arc uniqueness testing works correctly.
        // This must be done DURING the time period we hold the map lock,
        // to prevent races.
        //
        // Note that the permit may be absent.
        // This signifies that the future was aborted before a permit could be acquired.
        // Even though we failed to get a permit,
        // we still have a responisibility to potentially clean up the resource if we are the last one.
        drop(permit);

        // Perform a uniqueness test on the Arc.
        let is_last = Arc::get_mut(entry.get_mut()).is_some();

        // If the Arc we have is the last one,
        // remove it to save memory.
        //
        // If this key gets locked again,
        // a new entry will be created.
        if is_last {
            entry.remove();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn sanity() {
        let map = AsyncMutexMap::new();

        let guard_1 = map.lock(1).await;

        // Resource held, this should fail.
        map.try_lock(1).unwrap_err();

        // Resource not held, should succed.
        let guard_2 = map.try_lock(2).unwrap();

        // Resource held, should fail.
        map.try_lock(1).unwrap_err();

        // Release 1.
        drop(guard_1);

        // Resource no longer held, should succeed.
        map.try_lock(1).unwrap();

        // Release 2.
        drop(guard_2);

        // Map should have released all resources.
        assert!(map.map.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn timeout_cleanup() {
        let map = AsyncMutexMap::new();

        {
            let guard_1 = map.lock(1).await;

            let mut lock_future = std::pin::pin!(map.lock(1));
            tokio::time::timeout(Duration::from_millis(50), &mut lock_future)
                .await
                .unwrap_err();

            drop(guard_1);
            drop(lock_future);
        }

        // Map should have released all resources.
        assert!(map.map.lock().unwrap().is_empty());
    }
}

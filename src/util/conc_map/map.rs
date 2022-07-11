use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    sync::{
        Mutex as StdMutex, MutexGuard as StdMutexGuard, RwLock as StdRwLock,
        RwLockReadGuard as StdReadGuard, RwLockWriteGuard as StdWriteGuard,
    },
};

use hashbrown::HashMap;
use tokio::sync::{
    Mutex as TokioMutex, MutexGuard as TokioMutexGuard, RwLock as TokioRwLock,
    RwLockReadGuard as TokioReadGuard, RwLockWriteGuard as TokioWriteGuard,
};

use super::{
    guard::Guard,
    iter::{AsyncMutexMapIter, AsyncRwLockMapIter, SyncRwLockMapIter},
    key::MultMapKey,
    AsyncMutex, AsyncRwLock, MapLock, SyncMutex, SyncMutexMapIter, SyncRwLock,
};

/// [`MultMap`] that operates on the std library's [`RwLock`](std::sync::RwLock).
pub type SyncRwLockMap<K, V, const N: usize = 10> = MultMap<K, V, SyncRwLock, N>;

/// [`MultMap`] that operates on the std library's [`Mutex`](std::sync::RwLock).
pub type SyncMutexMap<K, V, const N: usize = 10> = MultMap<K, V, SyncMutex, N>;

/// [`MultMap`] that operates on tokio's [`RwLock`](tokio::sync::RwLock).
pub type AsyncRwLockMap<K, V, const N: usize = 10> = MultMap<K, V, AsyncRwLock, N>;

/// [`MultMap`] that operates on tokio's [`Mutex`](tokio::sync::RwLock).
pub type AsyncMutexMap<K, V, const N: usize = 10> = MultMap<K, V, AsyncMutex, N>;

/// Concurrent map with a similar internal structure to DashMap.
///
/// However, the amount of shards for this map can be set manually
/// and can also be locked through any locks like a sync RwLock or an async Mutex.
///
/// Access to the map is generally abstracted through a [`Guard`] which you get
/// from either `MultMap::read` or `MultMap::write`.
///
/// # DEADLOCKS
///
/// Note that the map can still deadlock when you hold a write-guard and want to
/// get another guard while both happen to fall into the same shard.
/// So don't do that :)
pub struct MultMap<K, V, L, const N: usize>
where
    L: MapLock<HashMap<K, V>>,
{
    pub(super) inner: [L::Lock; N],
}

impl<K: MultMapKey, V, const N: usize> SyncRwLockMap<K, V, N> {
    /// Acquire read access to a shard.
    ///
    /// # DEADLOCKS
    ///
    /// While not as bad as write access, you should still try to hold this guard
    /// as briefly as possible. If a write guard is being acquired while this guard
    /// is being held we got ourselves a potential deadlock.
    pub fn read(&self, key: K) -> Guard<StdReadGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].read().unwrap(), key)
    }

    /// Acquire write access to a shard.
    ///
    /// # DEADLOCKS
    ///
    /// Be sure you hold the guard as briefly as possible so that nothing deadlocks.
    /// If any guard for the same shard is being acquired while this guard is being held, that's no bueno.
    pub fn write(&self, key: K) -> Guard<StdWriteGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].write().unwrap(), key)
    }

    /// Check if all shards are empty.
    ///
    /// # DEADLOCKS
    ///
    /// This method will acquire read access to the shards so be sure you don't
    /// have a write-guard lying around unless you intend to deadlock yourself.
    pub fn is_empty(&self) -> bool {
        self.inner.iter().all(|map| map.read().unwrap().is_empty())
    }

    /// Returns an iterator to iterate over all elements of all shards.
    ///
    /// # DEADLOCKS
    ///
    /// While iterating, be sure there is no write-guard around or it will deadlock.
    pub fn iter(&self) -> SyncRwLockMapIter<'_, K, V, N> {
        SyncRwLockMapIter::new(self)
    }
}

impl<K: MultMapKey, V, const N: usize> SyncMutexMap<K, V, N> {
    /// Acquire sole access to a shard.
    ///
    /// # DEADLOCKS
    ///
    /// Hold the guard as briefly as possible since all other acquisations of this lock
    /// on the same shard will block until the guard is dropped.
    pub fn lock(&self, key: K) -> Guard<StdMutexGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].lock().unwrap(), key)
    }

    /// Check if all shards are empty.
    ///
    /// # DEADLOCKS
    ///
    /// This method will acquire read access to the shards so be sure you don't
    /// have a write-guard lying around unless you intend to deadlock yourself.
    pub fn is_empty(&self) -> bool {
        self.inner.iter().all(|map| map.lock().unwrap().is_empty())
    }

    /// Returns an iterator to iterate over all elements of all shards.
    ///
    /// # DEADLOCKS
    ///
    /// While iterating, be sure there is no write-guard around or it will deadlock.
    pub fn iter(&self) -> SyncMutexMapIter<'_, K, V, N> {
        SyncMutexMapIter::new(self)
    }
}

impl<K: MultMapKey, V, const N: usize> AsyncRwLockMap<K, V, N> {
    /// Acquire read access to a shard.
    ///
    /// # DEADLOCKS
    ///
    /// While not as bad as write access, you should still try to hold this guard
    /// as briefly as possible. If a write guard is being acquired while this guard
    /// is being held we got ourselves a potential deadlock.
    pub async fn read(&self, key: K) -> Guard<TokioReadGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].read().await, key)
    }

    /// Acquire write access to a shard.
    ///
    /// # DEADLOCKS
    ///
    /// Be sure you hold the guard as briefly as possible so that nothing deadlocks.
    /// If any guard for the same shard is being acquired while this guard is being held, that's no bueno.
    pub async fn write(&self, key: K) -> Guard<TokioWriteGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].write().await, key)
    }

    /// Check if all shards are empty.
    ///
    /// # DEADLOCKS
    ///
    /// This method will acquire read access to the shards so be sure you don't
    /// have a write-guard lying around unless you intend to deadlock yourself.
    pub async fn is_empty(&self) -> bool {
        for map in self.inner.iter() {
            if !map.read().await.is_empty() {
                return false;
            }
        }

        true
    }

    /// Returns a stream to iterate over all elements of all shards.
    ///
    /// # DEADLOCKS
    ///
    /// While iterating, be sure there is no write-guard around or it will deadlock.
    pub async fn iter(&self) -> AsyncRwLockMapIter<'_, K, V, N> {
        AsyncRwLockMapIter::new(self).await
    }
}

impl<K: MultMapKey, V, const N: usize> AsyncMutexMap<K, V, N> {
    /// Acquire sole access to a shard.
    ///
    /// # DEADLOCKS
    ///
    /// Hold the guard as briefly as possible since all other acquisations of this lock
    /// on the same shard will block until the guard is dropped.
    pub async fn lock(&self, key: K) -> Guard<TokioMutexGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].lock().await, key)
    }

    /// Check if all shards are empty.
    ///
    /// # DEADLOCKS
    ///
    /// This method will acquire read access to the shards so be sure you don't
    /// have a guard lying around unless you intend to deadlock yourself.
    pub async fn is_empty(&self) -> bool {
        for map in self.inner.iter() {
            if !map.lock().await.is_empty() {
                return false;
            }
        }

        true
    }

    /// Returns a stream to iterate over all elements of all shards.
    ///
    /// # DEADLOCKS
    ///
    /// While iterating, be sure there is no guard around or it will deadlock.
    pub async fn iter(&self) -> AsyncMutexMapIter<'_, K, V, N> {
        AsyncMutexMapIter::new(self).await
    }
}

impl<K, V, const N: usize> Default for SyncRwLockMap<K, V, N> {
    #[inline]
    fn default() -> Self {
        let inner = [(); N].map(|_| StdRwLock::new(HashMap::default()));

        Self { inner }
    }
}

impl<K, V, const N: usize> Default for SyncMutexMap<K, V, N> {
    #[inline]
    fn default() -> Self {
        let inner = [(); N].map(|_| StdMutex::new(HashMap::default()));

        Self { inner }
    }
}

impl<K, V, const N: usize> Default for AsyncRwLockMap<K, V, N> {
    #[inline]
    fn default() -> Self {
        let inner = [(); N].map(|_| TokioRwLock::new(HashMap::default()));

        Self { inner }
    }
}

impl<K, V, const N: usize> Default for AsyncMutexMap<K, V, N> {
    #[inline]
    fn default() -> Self {
        let inner = [(); N].map(|_| TokioMutex::new(HashMap::default()));

        Self { inner }
    }
}

impl<K, V, const N: usize> Debug for SyncRwLockMap<K, V, N>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if f.alternate() {
            let mut iter = self.inner.iter();

            if let Some(inner) = iter.next() {
                let locked = inner.read().unwrap();
                writeln!(f, "{locked:?}")?;

                for inner in iter {
                    let locked = inner.read().unwrap();
                    writeln!(f, "{locked:?}")?;
                }
            }

            Ok(())
        } else {
            let mut f = f.debug_map();

            for inner in self.inner.iter() {
                let locked = inner.read().unwrap();
                f.entries(locked.iter());
            }

            f.finish()
        }
    }
}

impl<K, V, const N: usize> Debug for SyncMutexMap<K, V, N>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if f.alternate() {
            let mut iter = self.inner.iter();

            if let Some(inner) = iter.next() {
                let locked = inner.lock().unwrap();
                writeln!(f, "{locked:?}")?;

                for inner in iter {
                    let locked = inner.lock().unwrap();
                    writeln!(f, "{locked:?}")?;
                }
            }

            Ok(())
        } else {
            let mut f = f.debug_map();

            for inner in self.inner.iter() {
                let locked = inner.lock().unwrap();
                f.entries(locked.iter());
            }

            f.finish()
        }
    }
}

impl<K, V, const N: usize> Debug for AsyncRwLockMap<K, V, N>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if f.alternate() {
            let mut iter = self.inner.iter();

            if let Some(inner) = iter.next() {
                match inner.try_read() {
                    Ok(locked) => write!(f, "{locked:?}")?,
                    Err(_) => write!(f, "<locked>")?,
                }

                for inner in iter {
                    match inner.try_read() {
                        Ok(locked) => write!(f, "\n{locked:?}")?,
                        Err(_) => write!(f, "\n<locked>")?,
                    }
                }
            }

            Ok(())
        } else {
            let mut locked = 0;
            let mut f_map = f.debug_map();

            for inner in self.inner.iter() {
                match inner.try_read() {
                    Ok(locked) => {
                        f_map.entries(locked.iter());
                    }
                    Err(_) => locked += 1,
                }
            }

            f_map.finish()?;

            if locked > 0 {
                write!(f, "{locked} inner maps were locked")?;
            }

            Ok(())
        }
    }
}

impl<K, V, const N: usize> Debug for AsyncMutexMap<K, V, N>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if f.alternate() {
            let mut iter = self.inner.iter();

            if let Some(inner) = iter.next() {
                match inner.try_lock() {
                    Ok(locked) => write!(f, "{locked:?}")?,
                    Err(_) => write!(f, "<locked>")?,
                }

                for inner in iter {
                    match inner.try_lock() {
                        Ok(locked) => write!(f, "\n{locked:?}")?,
                        Err(_) => write!(f, "\n<locked>")?,
                    }
                }
            }

            Ok(())
        } else {
            let mut locked = 0;
            let mut f_map = f.debug_map();

            for inner in self.inner.iter() {
                match inner.try_lock() {
                    Ok(locked) => {
                        f_map.entries(locked.iter());
                    }
                    Err(_) => locked += 1,
                }
            }

            f_map.finish()?;

            if locked > 0 {
                write!(f, "{locked} inner maps were locked")?;
            }

            Ok(())
        }
    }
}

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
    guard::Guard, key::MultMapKey, AsyncMutex, AsyncRwLock, MapLock, SyncMutex, SyncRwLock,
};

pub type SyncRwLockMap<K, V, const N: usize = 10> = MultMap<K, V, SyncRwLock, N>;
pub type SyncMutexMap<K, V, const N: usize = 10> = MultMap<K, V, SyncMutex, N>;
pub type AsyncRwLockMap<K, V, const N: usize = 10> = MultMap<K, V, AsyncRwLock, N>;
pub type AsyncMutexMap<K, V, const N: usize = 10> = MultMap<K, V, AsyncMutex, N>;

pub struct MultMap<K, V, L, const N: usize>
where
    L: MapLock<HashMap<K, V>>,
{
    inner: [L::Lock; N],
}

impl<K: MultMapKey, V, const N: usize> SyncRwLockMap<K, V, N> {
    pub fn read(&self, key: K) -> Guard<StdReadGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].read().unwrap(), key)
    }

    pub fn write(&self, key: K) -> Guard<StdWriteGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].write().unwrap(), key)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.iter().all(|map| map.read().unwrap().is_empty())
    }
}

impl<K: MultMapKey, V, const N: usize> SyncMutexMap<K, V, N> {
    pub fn lock(&self, key: K) -> Guard<StdMutexGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].lock().unwrap(), key)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.iter().all(|map| map.lock().unwrap().is_empty())
    }
}

impl<K: MultMapKey, V, const N: usize> AsyncRwLockMap<K, V, N> {
    pub async fn read(&self, key: K) -> Guard<TokioReadGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].read().await, key)
    }

    pub async fn write(&self, key: K) -> Guard<TokioWriteGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].write().await, key)
    }

    pub async fn is_empty(&self) -> bool {
        for map in self.inner.iter() {
            if !map.read().await.is_empty() {
                return false;
            }
        }

        true
    }
}

impl<K: MultMapKey, V, const N: usize> AsyncMutexMap<K, V, N> {
    pub async fn lock(&self, key: K) -> Guard<TokioMutexGuard<'_, HashMap<K, V>>, K, V> {
        Guard::new(self.inner[key.index::<N>()].lock().await, key)
    }

    pub async fn is_empty(&self) -> bool {
        for map in self.inner.iter() {
            if !map.lock().await.is_empty() {
                return false;
            }
        }

        true
    }
}

impl<K, V, const N: usize> Default for SyncRwLockMap<K, V, N> {
    fn default() -> Self {
        let inner = [(); N].map(|_| StdRwLock::new(HashMap::default()));

        Self { inner }
    }
}

impl<K, V, const N: usize> Default for SyncMutexMap<K, V, N> {
    fn default() -> Self {
        let inner = [(); N].map(|_| StdMutex::new(HashMap::default()));

        Self { inner }
    }
}

impl<K, V, const N: usize> Default for AsyncRwLockMap<K, V, N> {
    fn default() -> Self {
        let inner = [(); N].map(|_| TokioRwLock::new(HashMap::default()));

        Self { inner }
    }
}

impl<K, V, const N: usize> Default for AsyncMutexMap<K, V, N> {
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
            for inner in self.inner.iter() {
                let locked = inner.read().unwrap();
                writeln!(f, "{locked:?}")?;
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
            for inner in self.inner.iter() {
                let locked = inner.lock().unwrap();
                writeln!(f, "{locked:?}")?;
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
            for inner in self.inner.iter() {
                match inner.try_read() {
                    Ok(locked) => writeln!(f, "{locked:?}")?,
                    Err(_) => writeln!(f, "<locked>")?,
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
            for inner in self.inner.iter() {
                match inner.try_lock() {
                    Ok(locked) => writeln!(f, "{locked:?}")?,
                    Err(_) => writeln!(f, "<locked>")?,
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

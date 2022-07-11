use std::{
    mem,
    pin::Pin,
    sync::{MutexGuard as StdMutexGuard, RwLockReadGuard as StdReadGuard},
    task::{Context, Poll},
};

use futures::Stream;
use hashbrown::{hash_map::Iter, HashMap};
use tokio::sync::{MutexGuard as TokioMutexGuard, RwLockReadGuard as TokioReadGuard};

use super::{map::MultMap, AsyncMutex, AsyncRwLock, SyncMutex, SyncRwLock};

pub struct SyncRwLockMapIter<'m, K, V, const N: usize> {
    idx: usize,
    map: &'m MultMap<K, V, SyncRwLock, N>,
    guard: Option<StdReadGuard<'m, HashMap<K, V>>>,
    iter: Option<Iter<'m, K, V>>,
}

impl<'m, K, V, const N: usize> SyncRwLockMapIter<'m, K, V, N> {
    pub(super) fn new(map: &'m MultMap<K, V, SyncRwLock, N>) -> Self {
        let guard = map.inner[0].read().unwrap();
        let iter = unsafe { mem::transmute(guard.iter()) };

        Self {
            idx: 0,
            map,
            guard: Some(guard),
            iter: Some(iter),
        }
    }
}

impl<'m, K, V, const N: usize> Iterator for SyncRwLockMapIter<'m, K, V, N> {
    type Item = (&'m K, &'m V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut iter) = self.iter {
                if let Some(next) = iter.next() {
                    return Some(next);
                }
            }

            self.idx += 1;

            let inner = self.map.inner.get(self.idx)?;
            let guard = self.guard.insert(inner.read().unwrap());
            self.iter = Some(unsafe { mem::transmute(guard.iter()) });
        }
    }
}

pub struct SyncMutexMapIter<'m, K, V, const N: usize> {
    idx: usize,
    map: &'m MultMap<K, V, SyncMutex, N>,
    guard: Option<StdMutexGuard<'m, HashMap<K, V>>>,
    iter: Option<Iter<'m, K, V>>,
}

impl<'m, K, V, const N: usize> SyncMutexMapIter<'m, K, V, N> {
    pub(super) fn new(map: &'m MultMap<K, V, SyncMutex, N>) -> Self {
        let guard = map.inner[0].lock().unwrap();
        let iter = unsafe { mem::transmute(guard.iter()) };

        Self {
            idx: 0,
            map,
            guard: Some(guard),
            iter: Some(iter),
        }
    }
}

impl<'m, K, V, const N: usize> Iterator for SyncMutexMapIter<'m, K, V, N> {
    type Item = (&'m K, &'m V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut iter) = self.iter {
                if let Some(next) = iter.next() {
                    return Some(next);
                }
            }

            self.idx += 1;

            let inner = self.map.inner.get(self.idx)?;
            let guard = self.guard.insert(inner.lock().unwrap());
            self.iter = Some(unsafe { mem::transmute(guard.iter()) });
        }
    }
}

pub struct AsyncRwLockMapIter<'m, K, V, const N: usize> {
    idx: usize,
    map: &'m MultMap<K, V, AsyncRwLock, N>,
    guard: Option<TokioReadGuard<'m, HashMap<K, V>>>,
    iter: Option<Iter<'m, K, V>>,
}

impl<'m, K, V, const N: usize> AsyncRwLockMapIter<'m, K, V, N> {
    pub(super) async fn new(
        map: &'m MultMap<K, V, AsyncRwLock, N>,
    ) -> AsyncRwLockMapIter<'m, K, V, N> {
        let guard = map.inner[0].read().await;
        let iter = unsafe { mem::transmute(guard.iter()) };

        Self {
            idx: 0,
            map,
            guard: Some(guard),
            iter: Some(iter),
        }
    }
}

impl<'m, K, V, const N: usize> Stream for AsyncRwLockMapIter<'m, K, V, N> {
    type Item = (&'m K, &'m V);

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(ref mut iter) = self.iter {
                if let Some(next) = iter.next() {
                    return Poll::Ready(Some(next));
                }
            }

            self.idx += 1;

            let inner = match self.map.inner.get(self.idx) {
                Some(inner) => inner,
                None => return Poll::Ready(None),
            };

            let guard = match inner.try_read() {
                Ok(guard) => self.guard.insert(guard),
                Err(_) => return Poll::Pending,
            };

            self.iter = Some(unsafe { mem::transmute(guard.iter()) });
        }
    }
}

pub struct AsyncMutexMapIter<'m, K, V, const N: usize> {
    idx: usize,
    map: &'m MultMap<K, V, AsyncMutex, N>,
    guard: Option<TokioMutexGuard<'m, HashMap<K, V>>>,
    iter: Option<Iter<'m, K, V>>,
}

impl<'m, K, V, const N: usize> AsyncMutexMapIter<'m, K, V, N> {
    pub(super) async fn new(
        map: &'m MultMap<K, V, AsyncMutex, N>,
    ) -> AsyncMutexMapIter<'m, K, V, N> {
        let guard = map.inner[0].lock().await;
        let iter = unsafe { mem::transmute(guard.iter()) };

        Self {
            idx: 0,
            map,
            guard: Some(guard),
            iter: Some(iter),
        }
    }
}

impl<'m, K, V, const N: usize> Stream for AsyncMutexMapIter<'m, K, V, N> {
    type Item = (&'m K, &'m V);

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(ref mut iter) = self.iter {
                if let Some(next) = iter.next() {
                    return Poll::Ready(Some(next));
                }
            }

            self.idx += 1;

            let inner = match self.map.inner.get(self.idx) {
                Some(inner) => inner,
                None => return Poll::Ready(None),
            };

            let guard = match inner.try_lock() {
                Ok(guard) => self.guard.insert(guard),
                Err(_) => return Poll::Pending,
            };

            self.iter = Some(unsafe { mem::transmute(guard.iter()) });
        }
    }
}

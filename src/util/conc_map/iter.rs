use std::{
    mem,
    sync::{MutexGuard as StdMutexGuard, RwLockReadGuard as StdReadGuard},
};

use hashbrown::{hash_map::Iter, HashMap};

use super::{map::MultMap, SyncMutex, SyncRwLock};

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

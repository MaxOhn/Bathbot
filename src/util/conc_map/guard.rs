use std::{
    hash::{BuildHasher, Hash},
    marker::PhantomData,
    sync::{
        MutexGuard as StdMutexGuard, RwLockReadGuard as StdReadGuard,
        RwLockWriteGuard as StdWriteGuard,
    },
};

use hashbrown::{hash_map::Entry, HashMap};
use tokio::sync::{
    MutexGuard as TokioMutexGuard, RwLockReadGuard as TokioReadGuard,
    RwLockWriteGuard as TokioWriteGuard,
};

pub struct Guard<G, K, V> {
    guard: G,
    key: K,
    value: PhantomData<V>,
}

impl<G, K, V> Guard<G, K, V> {
    pub(super) fn new(guard: G, key: K) -> Self {
        Self {
            guard,
            key,
            value: PhantomData,
        }
    }
}

macro_rules! read_guard_methods {
    ($($ty:ident),*) => {
        $(
            impl<K, V> Guard<$ty<'_, HashMap<K, V>>, K, V>
            where
                K: Eq + Hash,
            {
                pub fn get(&self) -> Option<&V> {
                    self.guard.get(&self.key)
                }
            }
        )*
    }
}

read_guard_methods!(StdReadGuard, TokioReadGuard);

macro_rules! write_guard_methods {
    ($($ty:ident),*) => {
        $(
            impl<K, V> Guard<$ty<'_, HashMap<K, V>>, K, V>
            where
                K: Copy + Eq + Hash,
            {
                pub fn get(&self) -> Option<&V> {
                    self.guard.get(&self.key)
                }

                pub fn get_mut(&mut self) -> Option<&mut V> {
                    self.guard.get_mut(&self.key)
                }

                pub fn insert(&mut self, value: V) {
                    self.guard.insert(self.key, value);
                }

                pub fn remove(&mut self) -> Option<V> {
                    self.guard.remove(&self.key)
                }
            }

            impl<K, V, S> Guard<$ty<'_, HashMap<K, V, S>>, K, V>
            where
                K: Copy + Eq + Hash,
                S: BuildHasher
            {
                pub fn entry(&mut self) -> Entry<'_, K, V, S> {
                    self.guard.entry(self.key)
                }
            }
        )*
    }
}

write_guard_methods!(
    StdWriteGuard,
    TokioWriteGuard,
    StdMutexGuard,
    TokioMutexGuard
);

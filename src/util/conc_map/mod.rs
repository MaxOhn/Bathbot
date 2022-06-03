use std::sync::{Mutex as StdMutex, RwLock as StdRwLock};

use tokio::sync::{Mutex as TokioMutex, RwLock as TokioRwLock};

pub use self::{
    guard::Guard,
    key::MultMapKey,
    map::{AsyncMutexMap, AsyncRwLockMap, SyncMutexMap, SyncRwLockMap},
};

mod guard;
mod key;
mod map;

pub trait MapLock<M> {
    type Lock;
}

pub struct SyncRwLock;
pub struct SyncMutex;
pub struct AsyncRwLock;
pub struct AsyncMutex;

impl<M> MapLock<M> for SyncRwLock {
    type Lock = StdRwLock<M>;
}

impl<M> MapLock<M> for SyncMutex {
    type Lock = StdMutex<M>;
}

impl<M> MapLock<M> for AsyncRwLock {
    type Lock = TokioRwLock<M>;
}

impl<M> MapLock<M> for AsyncMutex {
    type Lock = TokioMutex<M>;
}

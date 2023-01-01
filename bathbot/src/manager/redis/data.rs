use std::{marker::PhantomData, ops::Deref};

use rkyv::{Archive, Archived, Deserialize, Infallible};

#[derive(Clone)]
pub enum RedisData<T> {
    Original(T),
    Archived(ArchivedBytes<T>),
}

impl<T> RedisData<T> {
    pub(super) fn new(original: T) -> Self {
        Self::Original(original)
    }

    pub(super) fn new_archived(bytes: Vec<u8>) -> Self {
        Self::Archived(ArchivedBytes::new(bytes))
    }
}

impl<T> RedisData<T>
where
    T: Archive,
    Archived<T>: Deserialize<T, Infallible>,
{
    pub fn into_original(self) -> T {
        match self {
            RedisData::Original(data) => data,
            RedisData::Archived(data) => data.deserialize(),
        }
    }
}

#[derive(Clone)]
pub struct ArchivedBytes<T> {
    bytes: Vec<u8>,
    phantom: PhantomData<T>,
}

impl<T> ArchivedBytes<T> {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            phantom: PhantomData,
        }
    }
}

impl<T: Archive> Deref for ArchivedBytes<T> {
    type Target = Archived<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Bytes originate from redis which only stores valid archived data
        unsafe { rkyv::archived_root::<T>(&self.bytes) }
    }
}

impl<T> ArchivedBytes<T>
where
    T: Archive,
    Archived<T>: Deserialize<T, Infallible>,
{
    pub fn deserialize(&self) -> T {
        <Archived<T> as Deserialize<T, _>>::deserialize(self, &mut Infallible).unwrap()
    }
}

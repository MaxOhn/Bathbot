use bathbot_cache::model::CachedArchive;
use rkyv::{Archive, Archived, Deserialize, Infallible};

#[derive(Clone)]
pub enum RedisData<T> {
    Original(T),
    Archive(CachedArchive<T>),
}

impl<T> RedisData<T> {
    pub(super) fn new(original: T) -> Self {
        Self::Original(original)
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
            RedisData::Archive(data) => data.deserialize(),
        }
    }
}

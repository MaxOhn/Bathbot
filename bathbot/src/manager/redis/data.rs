use bathbot_cache::model::CachedArchive;
use rkyv::{Archive, Archived, Deserialize, Infallible};

#[derive(Clone)]
pub enum RedisData<O, A = O> {
    Original(O),
    Archive(CachedArchive<A>),
}

impl<O, A> RedisData<O, A> {
    pub(super) fn new(original: O) -> Self {
        Self::Original(original)
    }
}

impl<O> RedisData<O>
where
    O: Archive,
    Archived<O>: Deserialize<O, Infallible>,
{
    pub fn into_original(self) -> O {
        match self {
            RedisData::Original(data) => data,
            RedisData::Archive(data) => data.deserialize(),
        }
    }
}

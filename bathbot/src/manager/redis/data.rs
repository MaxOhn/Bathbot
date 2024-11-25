use bathbot_cache::model::CachedArchive;
use rkyv::{
    bytecheck::CheckBytes,
    rancor::{Panic, Strategy},
    validation::{archive::ArchiveValidator, Validator},
    Archive, Archived, Deserialize,
};

#[derive(Clone)]
pub enum RedisData<O, A = O> {
    Original(O),
    Archive(CachedArchive<A>),
}

impl<O, A> RedisData<O, A> {
    pub fn new(original: O) -> Self {
        Self::Original(original)
    }
}

impl<O, A> RedisData<O, A>
where
    A: Archive,
    Archived<A>: Deserialize<O, Strategy<(), Panic>>
        + for<'a> CheckBytes<Strategy<Validator<ArchiveValidator<'a>, ()>, Panic>>,
{
    pub fn into_original(self) -> O {
        match self {
            RedisData::Original(data) => data,
            RedisData::Archive(data) => data.deserialize_into(),
        }
    }
}

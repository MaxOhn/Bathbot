use std::time::Duration;

use eyre::Result;
use redlight::rkyv_util::id::{ArchivedId, IdRkyv};
use rkyv::{
    collections::util::{Entry, EntryAdapter},
    primitive::ArchivedU64,
    rancor::{BoxedError, Fallible},
    ser::{Allocator, Writer},
    util::AlignedVec,
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, SerializeWith, With},
    Place,
};
use twilight_model::id::{marker::GuildMarker, Id};

use crate::data::{serialize_using_arena_and_with, BathbotRedisData};

pub struct CacheGuildShards;

type Original = [(Id<GuildMarker>, u64)];
pub type ArchivedCacheGuildShards = ArchivedVec<Entry<ArchivedId<GuildMarker>, ArchivedU64>>;

impl ArchiveWith<Original> for CacheGuildShards {
    type Archived = ArchivedCacheGuildShards;
    type Resolver = VecResolver;

    fn resolve_with(field: &Original, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(field.len(), resolver, out);
    }
}

impl<S: Fallible + Allocator + Writer + ?Sized> SerializeWith<Original, S> for CacheGuildShards {
    fn serialize_with(field: &Original, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        let iter = field.iter().map(|(guild, shard)| {
            EntryAdapter::<_, _, With<_, IdRkyv>, u64>::new(With::<_, IdRkyv>::cast(guild), shard)
        });

        ArchivedVec::serialize_from_iter(iter, serializer)
    }
}

impl BathbotRedisData for CacheGuildShards {
    type Archived = ArchivedCacheGuildShards;
    type Original = Original;
    type With = Self;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(240));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(&data)
    }
}

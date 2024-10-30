use std::time::Duration;

use eyre::Result;
use redlight::rkyv_util::id::{ArchivedId, IdRkyvMap};
use rkyv::{rancor::BoxedError, util::AlignedVec, vec::ArchivedVec};
use twilight_model::id::{marker::GuildMarker, Id};

use crate::data::{serialize_using_arena_and_with, BathbotRedisData};

pub struct CacheMissAnalyzerGuilds;

impl BathbotRedisData for CacheMissAnalyzerGuilds {
    type Archived = ArchivedVec<ArchivedId<GuildMarker>>;
    type Original = [Id<GuildMarker>];
    type With = IdRkyvMap;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(240));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(&data)
    }
}

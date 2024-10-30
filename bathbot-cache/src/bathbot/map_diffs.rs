use std::time::Duration;

use bathbot_model::rkyv_util::SliceAsVec;
use bathbot_psql::model::osu::{ArchivedMapVersion, MapVersion};
use eyre::Result;
use rkyv::{rancor::BoxedError, util::AlignedVec, vec::ArchivedVec};

use crate::data::{serialize_using_arena_and_with, BathbotRedisData};

pub struct CacheMapDiffs;

impl BathbotRedisData for CacheMapDiffs {
    type Archived = ArchivedVec<ArchivedMapVersion>;
    type Original = [MapVersion];
    type With = SliceAsVec;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(30));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(data)
    }
}

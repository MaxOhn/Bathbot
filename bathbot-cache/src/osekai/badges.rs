use std::time::Duration;

use bathbot_model::{rkyv_util::SliceAsVec, ArchivedOsekaiBadge, OsekaiBadge};
use rkyv::{rancor::BoxedError, util::AlignedVec, vec::ArchivedVec};

use crate::data::{serialize_using_arena_and_with, BathbotRedisData};

pub struct CacheBadges;

impl BathbotRedisData for CacheBadges {
    type Archived = ArchivedVec<ArchivedOsekaiBadge>;
    type Original = [OsekaiBadge];
    type With = SliceAsVec;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(7200));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(data)
    }
}

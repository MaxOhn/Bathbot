use std::time::Duration;

use bathbot_model::{rkyv_util::SliceAsVec, ArchivedOsekaiMedal, OsekaiMedal};
use rkyv::{rancor::BoxedError, util::AlignedVec, vec::ArchivedVec};

use crate::data::{serialize_using_arena_and_with, BathbotRedisData};

pub struct CacheMedals;

impl BathbotRedisData for CacheMedals {
    type Archived = ArchivedVec<ArchivedOsekaiMedal>;
    type Original = [OsekaiMedal];
    type With = SliceAsVec;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(3600));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(data)
    }
}

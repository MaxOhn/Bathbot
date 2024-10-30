use std::time::Duration;

use bathbot_model::{ArchivedCountryRegions, CountryRegions};
use rkyv::{rancor::BoxedError, util::AlignedVec, with::Identity};

use crate::data::{serialize_using_arena, BathbotRedisData};

pub struct CacheCountryRegions;

impl BathbotRedisData for CacheCountryRegions {
    type Archived = ArchivedCountryRegions;
    type Original = CountryRegions;
    type With = Identity;

    // 12 hours
    const EXPIRE: Option<Duration> = Some(Duration::from_secs(43_200));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena(data)
    }
}

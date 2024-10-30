use std::time::Duration;

use bathbot_model::rosu_v2::ranking::ArchivedRankings;
use rkyv::{rancor::BoxedError, util::AlignedVec};

use crate::data::{serialize_using_arena_and_with, BathbotRedisData};

pub struct CachePpRanking;

impl BathbotRedisData for CachePpRanking {
    type Archived = ArchivedRankings;
    type Original = rosu_v2::prelude::Rankings;
    type With = bathbot_model::rosu_v2::ranking::Rankings;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(1800));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(&data)
    }
}

use std::time::Duration;

use bathbot_model::{ArchivedOsuStatsBestScores, OsuStatsBestScores};
use rkyv::{rancor::BoxedError, util::AlignedVec, with::Identity};

use crate::data::{serialize_using_arena, BathbotRedisData};

pub struct CacheOsuStatsBest;

impl BathbotRedisData for CacheOsuStatsBest {
    type Archived = ArchivedOsuStatsBestScores;
    type Original = OsuStatsBestScores;
    type With = Identity;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(3600));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena(data)
    }
}

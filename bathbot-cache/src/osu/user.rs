use std::time::Duration;

use rkyv::{rancor::BoxedError, util::AlignedVec};

use crate::data::{serialize_using_arena_and_with, BathbotRedisData};

pub struct CacheOsuUser;

impl BathbotRedisData for CacheOsuUser {
    type Archived = bathbot_model::rosu_v2::user::ArchivedUser;
    type Original = rosu_v2::prelude::UserExtended;
    type With = bathbot_model::rosu_v2::user::User;

    // 10 minutes
    const EXPIRE: Option<Duration> = Some(Duration::from_secs(600));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(data)
    }
}

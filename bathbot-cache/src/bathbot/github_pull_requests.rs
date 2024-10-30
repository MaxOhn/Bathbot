use std::time::Duration;

use bathbot_model::{ArchivedPullRequests, PullRequests};
use eyre::Result;
use rkyv::{rancor::BoxedError, util::AlignedVec, with::Identity};

use crate::data::{serialize_using_arena, BathbotRedisData};

pub struct CacheGithubPullRequests;

impl BathbotRedisData for CacheGithubPullRequests {
    type Archived = ArchivedPullRequests;
    type Original = PullRequests;
    type With = Identity;

    // 30 minutes
    const EXPIRE: Option<Duration> = Some(Duration::from_secs(1800));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena(data)
    }
}

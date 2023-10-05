use std::{num::NonZeroU32, vec::IntoIter};

use serde::{Deserialize, Deserializer};
use time::OffsetDateTime;

use crate::deser::datetime_rfc3339;

#[derive(Clone, Copy, Deserialize, Debug)]
pub struct RespektiveUserRankHighest {
    pub rank: u32,
    #[serde(with = "datetime_rfc3339")]
    pub updated_at: OffsetDateTime,
}
#[derive(Deserialize, Debug)]
pub struct RespektiveUser {
    #[serde(deserialize_with = "zero_as_none")]
    pub rank: Option<NonZeroU32>,
    pub user_id: u32,
    #[serde(rename = "score")]
    pub ranked_score: u64,
    pub rank_highest: Option<RespektiveUserRankHighest>,
}

pub struct RespektiveUsers {
    inner: IntoIter<RespektiveUser>,
}

impl From<Vec<RespektiveUser>> for RespektiveUsers {
    fn from(users: Vec<RespektiveUser>) -> Self {
        Self {
            inner: users.into_iter(),
        }
    }
}

impl Iterator for RespektiveUsers {
    type Item = Option<RespektiveUser>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|user| (user.rank.is_some() || user.rank_highest.is_some()).then_some(user))
    }
}

fn zero_as_none<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NonZeroU32>, D::Error> {
    u32::deserialize(d).map(NonZeroU32::new)
}

use std::vec::IntoIter;

use serde::Deserialize;
use time::OffsetDateTime;

use crate::deser::datetime_rfc3339;

#[derive(Deserialize)]
pub struct RespektiveUserRankHighest {
    pub rank: u32,
    #[serde(with = "datetime_rfc3339")]
    pub updated_at: OffsetDateTime,
}
#[derive(Deserialize)]
pub struct RespektiveUser {
    pub rank: u32,
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
            .map(|user| (user.rank > 0).then_some(user))
    }
}

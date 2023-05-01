use std::vec::IntoIter;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct RespektiveUser {
    pub rank: u32,
    pub user_id: u32,
    #[serde(rename = "score")]
    pub ranked_score: u64,
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

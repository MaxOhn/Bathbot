use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, Error, FromRow, Row};

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Hash)]
pub struct StreamTrack {
    pub channel_id: u64,
    pub user_id: u64,
}

impl StreamTrack {
    pub fn new(channel_id: u64, user_id: u64) -> Self {
        Self {
            channel_id,
            user_id,
        }
    }
}

impl<'c> FromRow<'c, PgRow> for StreamTrack {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(StreamTrack {
            channel_id: row.get::<i64, _>(0) as u64,
            user_id: row.get::<i64, _>(1) as u64,
        })
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct TwitchUser {
    user_id: u64,
    name: String,
}

impl<'c> FromRow<'c, PgRow> for TwitchUser {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(TwitchUser {
            user_id: row.get::<i64, _>(0) as u64,
            name: row.get(1),
        })
    }
}

impl TwitchUser {
    pub fn new(user_id: u64, name: String) -> Self {
        Self { user_id, name }
    }
}

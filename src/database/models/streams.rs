use serde::{Deserialize, Serialize};

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

// impl<'c> FromRow<'c, MySqlRow> for StreamTrack {
//     fn from_row(row: &MySqlRow) -> Result<StreamTrack, sqlx::Error> {
//         Ok(StreamTrack {
//             channel_id: row.get("channel_id"),
//             user_id: row.get("user_id"),
//         })
//     }
// }

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct TwitchUser {
    user_id: u64,
    name: String,
}

impl TwitchUser {
    pub fn new(user_id: u64, name: String) -> Self {
        Self { user_id, name }
    }
}

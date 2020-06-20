use sqlx::{mysql::MySqlRow, FromRow, Row};

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Platform {
    Twitch = 0,
    Mixer = 1,
}

impl From<u8> for Platform {
    fn from(n: u8) -> Self {
        match n {
            0 => Self::Twitch,
            1 => Self::Mixer,
            _ => panic!("Cannot create Platform enum out of u8 {}", n),
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct StreamTrack {
    pub channel_id: u64,
    pub user_id: u64,
    pub platform: Platform,
}

impl StreamTrack {
    pub fn new(channel_id: u64, user_id: u64, platform: Platform) -> Self {
        Self {
            channel_id,
            user_id,
            platform,
        }
    }
}

impl<'c> FromRow<'c, MySqlRow> for StreamTrack {
    fn from_row(row: &MySqlRow) -> Result<StreamTrack, sqlx::Error> {
        let platform: u8 = row.get("platform");
        let platform = Platform::from(platform);
        Ok(StreamTrack {
            channel_id: row.get("channel_id"),
            user_id: row.get("user_id"),
            platform,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct TwitchUser {
    user_id: u64,
    name: String,
}

impl TwitchUser {
    pub fn new(user_id: u64, name: String) -> Self {
        Self { user_id, name }
    }
}

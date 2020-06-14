use super::super::schema::{stream_tracks, twitch_users};
use diesel::{
    backend, deserialize,
    expression::Expression,
    serialize,
    sql_types::{Tinyint, Unsigned},
};
use std::io::Write;

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq, AsExpression, FromSqlRow)]
#[repr(u8)]
pub enum Platform {
    Twitch = 0,
    Mixer = 1,
}

impl<DB> deserialize::FromSql<Unsigned<Tinyint>, DB> for Platform
where
    DB: backend::Backend,
    u8: deserialize::FromSql<Unsigned<Tinyint>, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> deserialize::Result<Self> {
        match u8::from_sql(bytes)? {
            0 => Ok(Platform::Twitch),
            1 => Ok(Platform::Mixer),
            x => Err(format!("Unrecognized variant {}", x).into()),
        }
    }
}

impl<DB> serialize::ToSql<Unsigned<Tinyint>, DB> for Platform
where
    DB: backend::Backend,
    u8: serialize::ToSql<Unsigned<Tinyint>, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut serialize::Output<W, DB>) -> serialize::Result {
        (*self as u8).to_sql(out)
    }
}

impl Expression for Platform {
    type SqlType = Unsigned<Tinyint>;
}

impl Into<u8> for Platform {
    fn into(self) -> u8 {
        self as u8
    }
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

#[derive(Copy, Clone, Insertable, Queryable, Identifiable, Debug, PartialEq, Associations)]
#[table_name = "stream_tracks"]
#[belongs_to(TwitchUser, foreign_key = "user_id")]
pub struct StreamTrackDB {
    id: u32,
    channel_id: u64,
    user_id: u64,
    platform: Platform,
}

impl StreamTrackDB {
    pub fn new(id: u32, channel_id: u64, user_id: u64, platform: impl Into<Platform>) -> Self {
        Self {
            id,
            channel_id,
            user_id,
            platform: platform.into(),
        }
    }
}

impl Into<StreamTrack> for StreamTrackDB {
    fn into(self) -> StreamTrack {
        StreamTrack::new(self.channel_id, self.user_id, self.platform)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
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

#[derive(Insertable, Queryable, Identifiable, Debug, PartialEq, Associations)]
#[primary_key(user_id)]
pub struct TwitchUser {
    user_id: u64,
    name: String,
}

impl TwitchUser {
    pub fn new(user_id: u64, name: String) -> Self {
        Self { user_id, name }
    }
}

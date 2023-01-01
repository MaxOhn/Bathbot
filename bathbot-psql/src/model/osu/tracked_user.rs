use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;
use twilight_model::id::{marker::ChannelMarker, Id};

type Channels<S> = HashMap<Id<ChannelMarker>, u8, S>;

pub struct DbTrackedOsuUser {
    pub user_id: i32,
    pub gamemode: i16,
    pub channels: Vec<u8>,
    pub last_update: OffsetDateTime,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TrackedOsuUserKey {
    pub user_id: u32,
    pub mode: GameMode,
}

#[derive(Clone, Debug)]
pub struct TrackedOsuUserValue<S> {
    pub channels: Channels<S>,
    pub last_update: OffsetDateTime,
}

impl<S> From<DbTrackedOsuUser> for (TrackedOsuUserKey, TrackedOsuUserValue<S>)
where
    S: Default + BuildHasher,
{
    #[inline]
    fn from(user: DbTrackedOsuUser) -> Self {
        let DbTrackedOsuUser {
            user_id,
            gamemode,
            channels,
            last_update,
        } = user;

        // SAFETY: The bytes originate from the DB which only provides valid archived data
        let archived_channels = unsafe { rkyv::archived_root::<Channels<S>>(&channels) };
        let channels = archived_channels.deserialize(&mut Infallible).unwrap();

        let key = TrackedOsuUserKey {
            user_id: user_id as u32,
            mode: (gamemode as u8).into(),
        };

        let value = TrackedOsuUserValue {
            channels,
            last_update,
        };

        (key, value)
    }
}

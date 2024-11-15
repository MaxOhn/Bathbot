use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
    num::NonZeroU64,
};

use rkyv::{
    rancor::{Panic, ResultExt},
    with::{ArchiveWith, AsVec, With},
};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;

pub type Channels<S> = HashMap<NonZeroU64, u8, S>;

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

        let archived_channels =
            rkyv::access::<<AsVec as ArchiveWith<Channels<S>>>::Archived, Panic>(&channels)
                .always_ok();

        let channels = rkyv::api::deserialize_using::<_, _, Panic>(
            With::<_, AsVec>::cast(archived_channels),
            &mut (),
        )
        .always_ok();

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

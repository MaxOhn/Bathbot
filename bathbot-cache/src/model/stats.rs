use std::{
    ops::{Add, AddAssign},
    sync::Mutex,
};

use bb8_redis::{RedisConnectionManager, bb8::Pool, redis::AsyncCommands};
use eyre::{Result, WrapErr};

use crate::key::RedisKey;

#[derive(Clone, Debug, Default)]
pub struct CacheStats {
    pub channels: isize,
    pub guilds: isize,
    pub roles: isize,
    pub unavailable_guilds: isize,
    pub users: isize,
}

#[derive(Default)]
#[must_use]
pub struct CacheChange {
    pub channels: isize,
    pub guilds: isize,
    pub roles: isize,
    pub unavailable_guilds: isize,
    pub users: isize,
}

impl Add for CacheChange {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;

        self
    }
}

impl AddAssign for CacheChange {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        let Self {
            channels,
            guilds,
            roles,
            unavailable_guilds,
            users,
        } = rhs;

        self.channels += channels;
        self.guilds += guilds;
        self.roles += roles;
        self.unavailable_guilds += unavailable_guilds;
        self.users += users;
    }
}

pub(crate) struct CacheStatsInternal {
    inner: Mutex<CacheStats>,
}

impl CacheStatsInternal {
    pub(crate) async fn new(redis: &Pool<RedisConnectionManager>) -> Result<Self> {
        let mut conn = redis
            .get()
            .await
            .wrap_err("Failed to get redis connection")?;

        macro_rules! scard {
            ($name:ident) => {
                conn.scard(RedisKey::$name()).await.wrap_err(concat!(
                    "Failed to get ",
                    stringify!($name),
                    " cardinality"
                ))?
            };
        }

        let stats = CacheStats {
            channels: scard!(channels),
            guilds: scard!(guilds),
            roles: scard!(roles),
            unavailable_guilds: scard!(unavailable_guilds),
            users: scard!(users),
        };

        Ok(Self {
            inner: Mutex::new(stats),
        })
    }

    pub(crate) fn update(&self, change: &CacheChange) {
        let mut unlocked = self.inner.lock().unwrap();

        unlocked.channels += change.channels;
        unlocked.guilds += change.guilds;
        unlocked.roles += change.roles;
        unlocked.unavailable_guilds += change.unavailable_guilds;
        unlocked.users += change.users;
    }

    pub(crate) fn get(&self) -> CacheStats {
        self.inner.lock().unwrap().to_owned()
    }
}

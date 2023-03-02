use std::{
    ops::{Add, AddAssign},
    sync::Mutex,
};

#[derive(Default)]
pub(crate) struct CacheStatsInternal {
    inner: Mutex<CacheStats>,
}

#[derive(Clone, Default)]
pub struct CacheStats {
    pub channels: isize,
    pub guilds: isize,
    pub roles: isize,
    pub unavailable_guilds: isize,
    pub users: isize,
}

impl CacheStatsInternal {
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

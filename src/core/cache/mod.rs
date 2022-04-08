mod defrost;
mod error;
mod freeze;
mod permissions;

use std::{collections::HashMap, iter::FromIterator};

use eyre::Report;
use rkyv::{Archive, Deserialize, Serialize};
use twilight_cache_inmemory::{
    model::{CachedGuild, CachedMember},
    GuildResource, InMemoryCache, InMemoryCacheStats, ResourceType,
};
use twilight_gateway::{shard::ResumeSession, Event};
use twilight_model::{
    channel::{Channel, Message},
    guild::Role,
    id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
        Id,
    },
    user::{CurrentUser, User},
};

pub use self::{
    error::{CacheMiss, FreezeError},
    permissions::RolesLookup,
};

use self::error::{ColdResumeErrorKind, DefrostError, DefrostInnerError, FreezeInnerError};

use super::Redis;

type CacheResult<T> = Result<T, CacheMiss>;

const STORE_DURATION: usize = 240; // seconds

const DATA_KEY: &str = "data";
const GUILD_KEY_PREFIX: &str = "guild_chunk";
const USER_KEY_PREFIX: &str = "user_chunk";
const MEMBER_KEY_PREFIX: &str = "member_chunk";
const CHANNEL_KEY_PREFIX: &str = "channel_chunk";
const ROLE_KEY_PREFIX: &str = "role_chunk";
const CURRENT_USER_KEY: &str = "current_user";

pub struct Cache {
    inner: InMemoryCache,
}

impl Cache {
    pub async fn new(redis: &Redis) -> (Self, ResumeData) {
        let resource_types = ResourceType::CHANNEL
            | ResourceType::GUILD
            | ResourceType::MEMBER
            | ResourceType::ROLE
            | ResourceType::USER_CURRENT;

        let inner = InMemoryCache::builder()
            .message_cache_size(0)
            .resource_types(resource_types)
            .build();

        let cache = Self { inner };

        let resume_data = match cache.defrost(redis).await {
            Ok(resume_data) => resume_data,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to defrost cache");
                warn!("{report:?}");

                ResumeData::default()
            }
        };

        (cache, resume_data)
    }

    pub fn update(&self, event: &Event) {
        self.inner.update(event)
    }

    pub fn stats(&self) -> InMemoryCacheStats<'_> {
        self.inner.stats()
    }

    pub fn channel<F, T>(&self, channel: Id<ChannelMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&Channel) -> T,
    {
        let channel = self
            .inner
            .channel(channel)
            .ok_or(CacheMiss::Channel { channel })?;

        Ok(f(&channel))
    }

    pub fn current_user(&self) -> CacheResult<CurrentUser> {
        self.inner.current_user().ok_or(CacheMiss::CurrentUser)
    }

    pub fn guild<F, T>(&self, guild: Id<GuildMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&CachedGuild) -> T,
    {
        let guild = self.inner.guild(guild).ok_or(CacheMiss::Guild { guild })?;

        Ok(f(&guild))
    }

    pub fn member<F, T>(&self, guild: Id<GuildMarker>, user: Id<UserMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&CachedMember) -> T,
    {
        let member = self
            .inner
            .member(guild, user)
            .ok_or(CacheMiss::Member { guild, user })?;

        Ok(f(&member))
    }

    pub fn members<F, T, C>(&self, guild: Id<GuildMarker>, f: F) -> C
    where
        C: Default + FromIterator<T>,
        F: Fn(&Id<UserMarker>) -> T,
    {
        self.inner
            .guild_members(guild)
            .map_or_else(C::default, |entry| entry.iter().map(f).collect())
    }

    pub fn role<F, T>(&self, role: Id<RoleMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&GuildResource<Role>) -> T,
    {
        let role = self.inner.role(role).ok_or(CacheMiss::Role { role })?;

        Ok(f(&role))
    }

    pub fn user<F, T>(&self, user: Id<UserMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&User) -> T,
    {
        let user = self.inner.user(user).ok_or(CacheMiss::User { user })?;

        Ok(f(&user))
    }

    pub fn is_guild_owner(
        &self,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    ) -> CacheResult<bool> {
        self.guild(guild, |g| g.owner_id() == user)
    }

    pub async fn is_own(&self, other: &Message) -> bool {
        match self.current_user() {
            Ok(user) => user.id == other.author.id,
            Err(_) => false,
        }
    }
}

type ResumeData = HashMap<u64, ResumeSession>;

#[derive(Archive, Deserialize, Serialize)]
struct ColdResumeData {
    resume_data: ResumeData,
    guild_chunks: usize,
    user_chunks: usize,
    member_chunks: usize,
    channel_chunks: usize,
    role_chunks: usize,
}

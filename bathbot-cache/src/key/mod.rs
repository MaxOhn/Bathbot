use std::borrow::Cow;

use bb8_redis::redis::{RedisWrite, ToRedisArgs};
use twilight_model::{
    channel::Channel,
    guild::{Guild, Member, Role},
    id::{
        Id,
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
    },
    user::User,
};

pub use self::to_key::ToCacheKey;
use self::{set::SetEntry, single::SingleEntry};

mod set;
mod single;
mod to_key;

#[derive(Clone, Debug)]
pub(crate) enum RedisKey<'a> {
    Single(SingleEntry),
    Set(SetEntry),
    Other(&'a [u8]),
}

impl RedisKey<'_> {
    pub(crate) fn channel(guild: Option<Id<GuildMarker>>, channel: Id<ChannelMarker>) -> Self {
        Self::Single(SingleEntry::Channel { guild, channel })
    }

    pub(crate) const fn channels() -> Self {
        Self::Set(SetEntry::Channels)
    }

    pub(crate) const fn current_user() -> Self {
        Self::Single(SingleEntry::CurrentUser)
    }

    pub(crate) fn guild(guild: Id<GuildMarker>) -> Self {
        Self::Single(SingleEntry::Guild { guild })
    }

    pub(crate) const fn guilds() -> Self {
        Self::Set(SetEntry::Guilds)
    }

    pub(crate) fn guild_channels(guild: Id<GuildMarker>) -> Self {
        Self::Set(SetEntry::GuildChannels { guild })
    }

    pub(crate) fn guild_members(guild: Id<GuildMarker>) -> Self {
        Self::Set(SetEntry::GuildMembers { guild })
    }

    pub(crate) fn guild_roles(guild: Id<GuildMarker>) -> Self {
        Self::Set(SetEntry::GuildRoles { guild })
    }

    pub(crate) fn member(guild: Id<GuildMarker>, user: Id<UserMarker>) -> Self {
        Self::Single(SingleEntry::Member { guild, user })
    }

    pub(crate) const fn resume_data() -> Self {
        Self::Single(SingleEntry::ResumeData)
    }

    pub(crate) fn role(guild: Id<GuildMarker>, role: Id<RoleMarker>) -> Self {
        Self::Single(SingleEntry::Role { guild, role })
    }

    pub(crate) const fn roles() -> Self {
        Self::Set(SetEntry::Roles)
    }

    pub(crate) const fn unavailable_guilds() -> Self {
        Self::Set(SetEntry::UnavailableGuilds)
    }

    pub(crate) fn user(user: Id<UserMarker>) -> Self {
        Self::Single(SingleEntry::User { user })
    }

    pub(crate) const fn users() -> Self {
        Self::Set(SetEntry::Users)
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        match self {
            Self::Single(key) => key.to_bytes(),
            Self::Set(key) => key.to_bytes(),
            Self::Other(bytes) => Cow::Borrowed(bytes),
        }
    }
}

impl<'k, K: ToCacheKey + ?Sized> From<&'k K> for RedisKey<'k> {
    #[inline]
    fn from(value: &'k K) -> Self {
        Self::Other(value.to_key())
    }
}

impl From<&Channel> for RedisKey<'_> {
    #[inline]
    fn from(channel: &Channel) -> Self {
        Self::channel(channel.guild_id, channel.id)
    }
}

impl From<&Guild> for RedisKey<'_> {
    #[inline]
    fn from(guild: &Guild) -> Self {
        Self::guild(guild.id)
    }
}

impl From<(Id<GuildMarker>, &Member)> for RedisKey<'_> {
    #[inline]
    fn from((guild, member): (Id<GuildMarker>, &Member)) -> Self {
        Self::member(guild, member.user.id)
    }
}

impl From<(Id<GuildMarker>, &Role)> for RedisKey<'_> {
    #[inline]
    fn from((guild, role): (Id<GuildMarker>, &Role)) -> Self {
        Self::role(guild, role.id)
    }
}

impl From<&User> for RedisKey<'_> {
    #[inline]
    fn from(user: &User) -> Self {
        Self::user(user.id)
    }
}

impl ToRedisArgs for RedisKey<'_> {
    #[inline]
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        match self.to_bytes() {
            Cow::Borrowed(key) => key.write_redis_args(out),
            Cow::Owned(key) => key.write_redis_args(out),
        }
    }
}

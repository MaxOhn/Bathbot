use std::borrow::Cow;

use bb8_redis::redis::{RedisWrite, ToRedisArgs};
use itoa::Buffer;
use twilight_model::{
    channel::Channel,
    guild::{Guild, Member, Role},
    id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
        Id,
    },
    user::User,
};

#[derive(Clone, Debug)]
pub(crate) enum RedisKey {
    CurrentUser,
    Channel {
        guild: Option<Id<GuildMarker>>,
        channel: Id<ChannelMarker>,
    },
    Guild {
        guild: Id<GuildMarker>,
    },
    Member {
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    },
    ResumeData,
    Role {
        guild: Id<GuildMarker>,
        role: Id<RoleMarker>,
    },
    User {
        user: Id<UserMarker>,
    },
}

impl RedisKey {
    const CURRENT_USER_PREFIX: &str = "CURRENT_USER";
    const GUILD_PREFIX: &str = "GUILD";
    const CHANNEL_PREFIX: &str = "CHANNEL";
    const MEMBER_PREFIX: &str = "MEMBER";
    const ROLE_PREFIX: &str = "ROLE";
    const USER_PREFIX: &str = "USER";

    const RESUME_DATA: &str = "RESUME_DATA";
    const CHANNEL_IDS: &str = "CHANNEL_IDS";
    const GUILD_IDS: &str = "GUILD_IDS";
    const ROLE_IDS: &str = "ROLE_IDS";
    const UNAVAILABLE_GUILD_IDS: &str = "UNAVAILABLE_GUILD_IDS";
    const USER_IDS: &str = "USER_IDS";

    pub(crate) fn channel(guild: Option<Id<GuildMarker>>, channel: Id<ChannelMarker>) -> Self {
        Self::Channel { guild, channel }
    }

    pub(crate) fn guild(guild: Id<GuildMarker>) -> Self {
        Self::Guild { guild }
    }

    pub(crate) fn member(guild: Id<GuildMarker>, user: Id<UserMarker>) -> Self {
        Self::Member { guild, user }
    }

    pub(crate) fn role(guild: Id<GuildMarker>, role: Id<RoleMarker>) -> Self {
        Self::Role { guild, role }
    }

    pub(crate) fn user(user: Id<UserMarker>) -> Self {
        Self::User { user }
    }

    pub(crate) fn guild_channels_key(guild: Id<GuildMarker>) -> String {
        format!("GUILD_CHANNELS:{guild}")
    }

    pub(crate) fn guild_members_key(guild: Id<GuildMarker>) -> String {
        format!("GUILD_MEMBERS:{guild}")
    }

    pub(crate) fn guild_roles_key(guild: Id<GuildMarker>) -> String {
        format!("GUILD_ROLES:{guild}")
    }

    pub(crate) fn channel_ids_key() -> &'static str {
        Self::CHANNEL_IDS
    }

    pub(crate) fn guild_ids_key() -> &'static str {
        Self::GUILD_IDS
    }

    pub(crate) fn role_ids_key() -> &'static str {
        Self::ROLE_IDS
    }

    pub(crate) fn unavailable_guild_ids_key() -> &'static str {
        Self::UNAVAILABLE_GUILD_IDS
    }

    pub(crate) fn user_ids_key() -> &'static str {
        Self::USER_IDS
    }

    fn to_str(&self) -> Cow<'static, [u8]> {
        // Using a Vec<u8> instead of String to optimize pushing single characters
        let mut res = Cow::default();

        fn push(res: &mut Vec<u8>, slice: &str) {
            res.extend_from_slice(slice.as_bytes());
        }

        match self {
            Self::CurrentUser => res = Cow::Borrowed(Self::CURRENT_USER_PREFIX.as_bytes()),
            Self::Channel { guild, channel } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                res.extend_from_slice(Self::CHANNEL_PREFIX.as_bytes());
                res.push(b':');

                match guild {
                    Some(guild) => {
                        push(res, buf.format(guild.get()));
                        res.push(b':');
                        push(res, buf.format(channel.get()));
                    }
                    None => push(res, buf.format(channel.get())),
                }
            }
            Self::Guild { guild } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, Self::GUILD_PREFIX);
                res.push(b':');
                push(res, buf.format(guild.get()));
            }
            Self::Member { guild, user } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, Self::MEMBER_PREFIX);
                res.push(b':');
                push(res, buf.format(guild.get()));
                res.push(b':');
                push(res, buf.format(user.get()));
            }
            Self::ResumeData => res = Cow::Borrowed(Self::RESUME_DATA.as_bytes()),
            Self::Role { guild, role } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, Self::ROLE_PREFIX);
                res.push(b':');
                push(res, buf.format(guild.get()));
                res.push(b':');
                push(res, buf.format(role.get()));
            }
            Self::User { user } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, Self::USER_PREFIX);
                res.push(b':');
                push(res, buf.format(user.get()));
            }
        }

        res
    }
}

impl From<&Channel> for RedisKey {
    #[inline]
    fn from(channel: &Channel) -> Self {
        Self::channel(channel.guild_id, channel.id)
    }
}

impl From<&Guild> for RedisKey {
    #[inline]
    fn from(guild: &Guild) -> Self {
        Self::guild(guild.id)
    }
}

impl From<(Id<GuildMarker>, &Member)> for RedisKey {
    #[inline]
    fn from((guild, member): (Id<GuildMarker>, &Member)) -> Self {
        Self::member(guild, member.user.id)
    }
}

impl From<(Id<GuildMarker>, &Role)> for RedisKey {
    #[inline]
    fn from((guild, role): (Id<GuildMarker>, &Role)) -> Self {
        Self::role(guild, role.id)
    }
}

impl From<&User> for RedisKey {
    #[inline]
    fn from(user: &User) -> Self {
        Self::user(user.id)
    }
}

impl ToRedisArgs for RedisKey {
    #[inline]
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        match self.to_str() {
            Cow::Borrowed(key) => key.write_redis_args(out),
            Cow::Owned(key) => key.write_redis_args(out),
        }
    }
}

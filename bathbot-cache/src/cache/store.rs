use bb8_redis::redis::AsyncCommands;
use eyre::{Result, WrapErr};
use twilight_model::{
    application::interaction::application_command::InteractionMember,
    channel::Channel,
    gateway::payload::incoming::MemberUpdate,
    guild::{Guild, Member, PartialGuild, PartialMember, Role},
    id::{marker::GuildMarker, Id},
    user::{CurrentUser, User},
};

use crate::{
    key::RedisKey,
    model::{CachedGuild, CachedMember},
    serializer::{MultiSerializer, SingleSerializer},
    util::{AlignedVecRedisArgs, Zipped},
    Cache,
};

impl Cache {
    pub(crate) async fn cache_channel(&self, channel: &Channel) -> Result<()> {
        let bytes = SingleSerializer::channel(channel)?;
        let mut conn = self.connection().await?;
        let key = RedisKey::from(channel);

        conn.set(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store channel bytes")?;

        if let Some(guild) = channel.guild_id {
            let guild_key = RedisKey::guild_channels_key(guild);

            conn.sadd(guild_key, channel.id.get())
                .await
                .wrap_err("Failed to add channel as guild channel")?;
        }

        conn.sadd(RedisKey::channel_ids_key(), channel.id.get())
            .await
            .wrap_err("Failed to add channel as channel id")?;

        Ok(())
    }

    pub(crate) async fn cache_channels(
        &self,
        guild: Id<GuildMarker>,
        channels: &[Channel],
    ) -> Result<()> {
        if channels.is_empty() {
            return Ok(());
        }

        let mut serializer = MultiSerializer::default();

        let (channels, channel_ids) = channels
            .iter()
            .map(|channel| {
                serializer.channel(channel).map(|bytes| {
                    let key = RedisKey::from(channel);

                    ((key, AlignedVecRedisArgs(bytes)), channel.id.get())
                })
            })
            .collect::<Result<Zipped<Vec<_>, Vec<_>>, _>>()?
            .into_parts();

        let mut conn = self.connection().await?;

        conn.set_multiple(&channels)
            .await
            .wrap_err("Failed to store channels bytes")?;

        let guild_key = RedisKey::guild_channels_key(guild);

        conn.sadd(guild_key, &channel_ids)
            .await
            .wrap_err("Failed to add users as guild members")?;

        conn.sadd(RedisKey::channel_ids_key(), &channel_ids)
            .await
            .wrap_err("Failed to add channels as channel ids")?;

        Ok(())
    }

    pub(crate) async fn cache_current_user(&self, user: &CurrentUser) -> Result<()> {
        let bytes = SingleSerializer::current_user(user)?;

        self.connection()
            .await?
            .set(RedisKey::CurrentUser, bytes.as_slice())
            .await
            .wrap_err("Failed to store current user bytes")?;

        Ok(())
    }

    pub(crate) async fn cache_guild(&self, guild: &Guild) -> Result<()> {
        self.cache_channels(guild.id, &guild.channels).await?;
        self.cache_channels(guild.id, &guild.threads).await?;
        self.cache_members(guild.id, &guild.members).await?;
        self.cache_roles(guild.id, &guild.roles).await?;

        let cached_guild = CachedGuild::from(guild);
        let bytes = SingleSerializer::guild(&cached_guild)?;
        let mut conn = self.connection().await?;
        let key = RedisKey::from(guild);

        conn.set(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store guild bytes")?;

        conn.sadd(RedisKey::guild_ids_key(), guild.id.get())
            .await
            .wrap_err("Failed to add guild as guild id")?;

        conn.srem(RedisKey::unavailable_guild_ids_key(), guild.id.get())
            .await
            .wrap_err("Failed to remove guild as unavailable guild id")?;

        Ok(())
    }

    pub(crate) async fn cache_interaction_member(
        &self,
        guild: Id<GuildMarker>,
        member: &InteractionMember,
        user: &User,
    ) -> Result<()> {
        let cached_member = CachedMember::from(member);

        self.cache_member_user(guild, &cached_member, user).await
    }

    pub(crate) async fn cache_member(&self, guild: Id<GuildMarker>, member: &Member) -> Result<()> {
        let cached_member = CachedMember::from(member);

        self.cache_member_user(guild, &cached_member, &member.user)
            .await
    }

    pub(crate) async fn cache_member_update(&self, update: &MemberUpdate) -> Result<()> {
        let cached_member = CachedMember::from(update);

        self.cache_member_user(update.guild_id, &cached_member, &update.user)
            .await
    }

    pub(crate) async fn cache_member_user(
        &self,
        guild: Id<GuildMarker>,
        member: &CachedMember<'_>,
        user: &User,
    ) -> Result<()> {
        let mut serializer = MultiSerializer::default();
        let member_bytes = serializer.member(member)?;
        let user_bytes = serializer.user(user)?;

        let mut conn = self.connection().await?;

        let items = &[
            (RedisKey::member(guild, user.id), member_bytes.as_slice()),
            (RedisKey::user(user.id), user_bytes.as_slice()),
        ];

        conn.set_multiple(items)
            .await
            .wrap_err("Failed to store member or user bytes")?;

        let guild_key = RedisKey::guild_members_key(guild);

        conn.sadd(guild_key, user.id.get())
            .await
            .wrap_err("Failed to add user as guild member")?;

        conn.sadd(RedisKey::user_ids_key(), user.id.get())
            .await
            .wrap_err("Failed to add user as user id")?;

        Ok(())
    }

    pub(crate) async fn cache_members(
        &self,
        guild: Id<GuildMarker>,
        members: &[Member],
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let mut serializer = MultiSerializer::default();

        let (zipped_members, users) = members
            .iter()
            .map(|member| {
                let user_id = member.user.id;

                let user = serializer
                    .user(&member.user)
                    .map(|bytes| (RedisKey::from(&member.user), AlignedVecRedisArgs(bytes)));

                let cached_member = CachedMember::from(member);

                let member = serializer.member(&cached_member).map(|bytes| {
                    let key = RedisKey::member(guild, member.user.id);

                    (key, AlignedVecRedisArgs(bytes))
                });

                match (member, user) {
                    (Ok(member), Ok(user)) => Ok(((member, user_id.get()), user)),
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            })
            .collect::<Result<Zipped<Zipped<Vec<_>, Vec<_>>, Vec<_>>>>()?
            .into_parts();

        let (members, member_ids) = zipped_members.into_parts();

        let mut conn = self.connection().await?;

        conn.set_multiple(&members)
            .await
            .wrap_err("Failed to store members bytes")?;

        conn.set_multiple(&users)
            .await
            .wrap_err("Failed to store users bytes")?;

        let guild_key = RedisKey::guild_members_key(guild);

        conn.sadd(guild_key, &member_ids)
            .await
            .wrap_err("Failed to add users as guild members")?;

        conn.sadd(RedisKey::user_ids_key(), &member_ids)
            .await
            .wrap_err("Failed to add users as user ids")?;

        Ok(())
    }

    pub(crate) async fn cache_partial_guild(&self, guild: &PartialGuild) -> Result<()> {
        self.cache_roles(guild.id, &guild.roles).await?;

        let mut conn = self.connection().await?;

        let cached_guild = CachedGuild::from(guild);
        let bytes = SingleSerializer::guild(&cached_guild)?;
        let key = RedisKey::guild(guild.id);

        conn.set(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store guild bytes")?;

        conn.sadd(RedisKey::guild_ids_key(), guild.id.get())
            .await
            .wrap_err("Failed to add guild as guild id")?;

        conn.srem(RedisKey::unavailable_guild_ids_key(), guild.id.get())
            .await
            .wrap_err("Failed to remove guild as unavailable guild id")?;

        Ok(())
    }

    pub(crate) async fn cache_partial_member(
        &self,
        guild_id: Id<GuildMarker>,
        member: &PartialMember,
        user: &User,
    ) -> Result<()> {
        let cached_member = CachedMember::from(member);

        self.cache_member_user(guild_id, &cached_member, user).await
    }

    pub(crate) async fn cache_role(&self, guild: Id<GuildMarker>, role: &Role) -> Result<()> {
        let bytes = SingleSerializer::role(role)?;
        let mut conn = self.connection().await?;
        let key = RedisKey::role(guild, role.id);

        conn.set(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store role bytes")?;

        let guild_key = RedisKey::guild_roles_key(guild);

        conn.sadd(guild_key, role.id.get())
            .await
            .wrap_err("Failed to add role as guild role")?;

        conn.sadd(RedisKey::role_ids_key(), role.id.get())
            .await
            .wrap_err("Failed to add role as role id")?;

        Ok(())
    }

    pub(crate) async fn cache_roles<'r, I>(&self, guild: Id<GuildMarker>, roles: I) -> Result<()>
    where
        I: IntoIterator<Item = &'r Role>,
    {
        let mut serializer = MultiSerializer::default();

        let (roles, role_ids) = roles
            .into_iter()
            .map(|role| {
                serializer.role(role).map(|bytes| {
                    let key = RedisKey::role(guild, role.id);

                    ((key, AlignedVecRedisArgs(bytes)), role.id.get())
                })
            })
            .collect::<Result<Zipped<Vec<_>, Vec<_>>, _>>()?
            .into_parts();

        if roles.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection().await?;

        conn.set_multiple(&roles)
            .await
            .wrap_err("Failed to store roles bytes")?;

        let guild_key = RedisKey::guild_roles_key(guild);

        conn.sadd(guild_key, &role_ids)
            .await
            .wrap_err("Failed to add roles as guild roles")?;

        conn.sadd(RedisKey::role_ids_key(), &role_ids)
            .await
            .wrap_err("Failed to add roles as role ids")?;

        Ok(())
    }

    pub(crate) async fn cache_unavailable_guild(&self, guild: Id<GuildMarker>) -> Result<()> {
        let mut conn = self.connection().await?;

        let is_moved: bool = conn
            .smove(
                RedisKey::guild_ids_key(),
                RedisKey::unavailable_guild_ids_key(),
                guild.get(),
            )
            .await
            .wrap_err("Failed to move guild id")?;

        if is_moved {
            conn.del(RedisKey::guild(guild))
                .await
                .wrap_err("Failed to delete guild entry")?;

            self.delete_guild_items(guild).await?;
        } else {
            conn.sadd(RedisKey::unavailable_guild_ids_key(), guild.get())
                .await
                .wrap_err("Failed to add guild to unavailable guilds")?;
        }

        Ok(())
    }

    pub(crate) async fn cache_user(&self, user: &User) -> Result<()> {
        let mut conn = self.connection().await?;

        let bytes = SingleSerializer::user(user)?;
        let key = RedisKey::from(user);

        conn.set(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store user bytes")?;

        conn.sadd(RedisKey::user_ids_key(), user.id.get())
            .await
            .wrap_err("Failed to add user as user id")?;

        Ok(())
    }
}

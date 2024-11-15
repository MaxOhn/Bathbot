use bathbot_model::twilight::{
    channel::CachedChannel,
    guild::{CachedGuild, CachedMember, CachedRole},
    user::{CachedCurrentUser, CachedUser},
};
use bb8_redis::redis::AsyncCommands;
use eyre::{Report, Result, WrapErr};
use rkyv::{
    rancor::{BoxedError, Strategy},
    ser::{allocator::ArenaHandle, Serializer},
    util::AlignedVec,
    with::{ArchiveWith, SerializeWith, With},
    Serialize,
};
use twilight_model::{
    application::interaction::application_command::InteractionMember,
    channel::Channel,
    gateway::payload::incoming::MemberUpdate,
    guild::{Guild, Member as TwMember, PartialGuild, PartialMember, Role},
    id::{marker::GuildMarker, Id},
    user::{CurrentUser, User},
};

use crate::{
    key::{RedisKey, ToCacheKey},
    model::{CacheChange, CacheConnection},
    util::{AlignedVecRedisArgs, Zipped},
    Cache,
};

type CacheSerializer<'a> = Serializer<AlignedVec<8>, ArenaHandle<'a>, ()>;

impl Cache {
    /// Serializes a values into bytes and stores them for the given key.
    pub async fn store<K, T>(
        conn: &mut CacheConnection<'_>,
        key: &K,
        value: &T,
        expire_seconds: u64,
    ) -> Result<()>
    where
        K: ToCacheKey + ?Sized,
        T: for<'a> Serialize<Strategy<CacheSerializer<'a>, BoxedError>>,
    {
        let bytes = rkyv::util::with_arena(|arena| {
            let mut serializer = Serializer::new(AlignedVec::new(), arena.acquire(), ());
            rkyv::api::serialize_using::<_, BoxedError>(value, &mut serializer)?;

            Ok::<_, BoxedError>(serializer.into_writer())
        })
        .wrap_err("Failed to serialize value")?;

        Self::store_raw(conn, key, bytes.as_slice(), expire_seconds).await
    }

    pub async fn store_raw<K>(
        CacheConnection(conn): &mut CacheConnection<'_>,
        key: &K,
        bytes: &[u8],
        expire_seconds: u64,
    ) -> Result<()>
    where
        K: ToCacheKey + ?Sized,
    {
        let key = RedisKey::from(key);

        conn.set_ex(key, bytes, expire_seconds)
            .await
            .map_err(Report::new)
    }

    /// Serializes a values into bytes, opens a new connection, and stores the
    /// bytes for the given key.
    ///
    /// **Note**: `Cache::store` should always be preferred if `Cache::fetch`
    /// was called beforehand.
    pub async fn store_new<K, T, const N: usize>(
        &self,
        key: &K,
        value: &T,
        expire_seconds: u64,
    ) -> Result<()>
    where
        K: ToCacheKey + ?Sized,
        T: for<'a> Serialize<Strategy<CacheSerializer<'a>, BoxedError>>,
    {
        let mut conn = CacheConnection(self.connection().await?);

        Self::store(&mut conn, key, value, expire_seconds).await
    }

    pub async fn store_new_raw<K>(&self, key: &K, bytes: &[u8], expire_seconds: u64) -> Result<()>
    where
        K: ToCacheKey + ?Sized,
    {
        let mut conn = CacheConnection(self.connection().await?);

        Self::store_raw(&mut conn, key, bytes, expire_seconds).await
    }

    /// Insert a value into a set.
    ///
    /// Returns whether the value was newly inserted. That is:
    ///
    /// - If the set did not previously contain this value, `true` is returned.
    /// - If the set already contained this value, `false` is returned.
    ///
    /// The currently only use is for values of type `u64`. If other use-cases
    /// arise, this type should be adjusted.
    pub async fn insert_into_set<K>(&self, key: &K, value: u64) -> Result<bool>
    where
        K: ToCacheKey + ?Sized,
    {
        let key = RedisKey::from(key);
        let count: u8 = self.connection().await?.sadd(key, value).await?;

        Ok(count == 1)
    }

    pub(crate) async fn cache_channel(&self, channel: &Channel) -> Result<CacheChange> {
        let bytes = rkyv::util::with_arena(|arena| {
            let mut serializer = Serializer::new(AlignedVec::<8>::new(), arena.acquire(), ());
            let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
            let with = With::<_, CachedChannel>::cast(channel);
            rkyv::api::serialize_using(with, strategy).wrap_err("Failed to serialize channel")?;

            Ok::<_, Report>(serializer.into_writer())
        })?;

        let mut conn = self.connection().await?;
        let key = RedisKey::from(channel);

        conn.set::<_, _, ()>(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store channel bytes")?;

        if let Some(guild) = channel.guild_id {
            let guild_key = RedisKey::guild_channels(guild);

            conn.sadd::<_, _, ()>(guild_key, channel.id.get())
                .await
                .wrap_err("Failed to add channel as guild channel")?;
        }

        let added: isize = conn
            .sadd(RedisKey::channels(), channel.id.get())
            .await
            .wrap_err("Failed to add channel as channel id")?;

        Ok(CacheChange {
            channels: added,
            ..Default::default()
        })
    }

    pub(crate) async fn cache_channels(
        &self,
        guild: Id<GuildMarker>,
        channels: &[Channel],
    ) -> Result<CacheChange> {
        if channels.is_empty() {
            return Ok(CacheChange::default());
        }

        let (channels, channel_ids) = rkyv::util::with_arena(|arena| {
            let mut serializer = Serializer::new(AlignedVec::<8>::new(), arena.acquire(), ());

            channels
                .iter()
                .map(move |channel| {
                    let bytes = {
                        let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
                        let with = With::<_, CachedChannel>::cast(channel);
                        rkyv::api::serialize_using(with, strategy)
                            .wrap_err("Failed to serialize channel")?;

                        let bytes = serializer.writer.as_slice().to_vec();
                        serializer.writer.clear();

                        bytes
                    };

                    let key = RedisKey::from(channel);

                    Ok::<_, Report>(((key, bytes), channel.id.get()))
                })
                .collect::<Result<Zipped<Vec<_>, Vec<_>>, _>>()
        })?
        .into_parts();

        let mut conn = self.connection().await?;

        conn.mset::<_, _, ()>(&channels)
            .await
            .wrap_err("Failed to store channels bytes")?;

        let guild_key = RedisKey::guild_channels(guild);

        conn.sadd::<_, _, ()>(guild_key, &channel_ids)
            .await
            .wrap_err("Failed to add users as guild members")?;

        let added: isize = conn
            .sadd(RedisKey::channels(), &channel_ids)
            .await
            .wrap_err("Failed to add channels as channel ids")?;

        Ok(CacheChange {
            channels: added,
            ..Default::default()
        })
    }

    pub(crate) async fn cache_current_user(&self, user: &CurrentUser) -> Result<()> {
        let bytes = {
            let mut serializer = AlignedVec::<8>::new();
            let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
            let with = With::<_, CachedCurrentUser<'_>>::cast(user);
            rkyv::api::serialize_using(with, strategy)
                .wrap_err("Failed to serialize current user")?;

            serializer
        };

        self.connection()
            .await?
            .set::<_, _, ()>(RedisKey::current_user(), bytes.as_slice())
            .await
            .wrap_err("Failed to store current user bytes")?;

        Ok(())
    }

    pub(crate) async fn cache_guild(&self, guild: &Guild) -> Result<CacheChange> {
        let channels_change = self.cache_channels(guild.id, &guild.channels).await?;
        let threads_change = self.cache_channels(guild.id, &guild.threads).await?;
        let members_change = self.cache_members(guild.id, &guild.members).await?;
        let roles_change = self.cache_roles(guild.id, &guild.roles).await?;

        let mut change = channels_change + threads_change + members_change + roles_change;

        let bytes = {
            let mut serializer = AlignedVec::<8>::new();
            let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
            let with = With::<_, CachedGuild>::cast(guild);
            rkyv::api::serialize_using(with, strategy).wrap_err("Failed to serialize guild")?;

            serializer
        };

        let mut conn = self.connection().await?;
        let key = RedisKey::from(guild);

        conn.set::<_, _, ()>(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store guild bytes")?;

        let guilds_added: isize = conn
            .sadd(RedisKey::guilds(), guild.id.get())
            .await
            .wrap_err("Failed to add guild as guild id")?;

        let unavailable_guilds_removed: isize = conn
            .srem(RedisKey::unavailable_guilds(), guild.id.get())
            .await
            .wrap_err("Failed to remove guild as unavailable guild id")?;

        change.guilds += guilds_added;
        change.unavailable_guilds -= unavailable_guilds_removed;

        Ok(change)
    }

    pub(crate) async fn cache_interaction_member(
        &self,
        guild: Id<GuildMarker>,
        member: &InteractionMember,
        user: &User,
    ) -> Result<CacheChange> {
        self.cache_member_user(guild, member, user).await
    }

    pub(crate) async fn cache_member(
        &self,
        guild: Id<GuildMarker>,
        member: &TwMember,
    ) -> Result<CacheChange> {
        self.cache_member_user(guild, member, &member.user).await
    }

    pub(crate) async fn cache_member_update(&self, update: &MemberUpdate) -> Result<CacheChange> {
        self.cache_member_user(update.guild_id, update, &update.user)
            .await
    }

    pub(crate) async fn cache_member_user<M>(
        &self,
        guild: Id<GuildMarker>,
        member: &M,
        user: &User,
    ) -> Result<CacheChange>
    where
        CachedMember: ArchiveWith<M>
            + for<'a> SerializeWith<
                M,
                Strategy<Serializer<AlignedVec<8>, ArenaHandle<'a>, ()>, BoxedError>,
            >,
    {
        async fn inner(
            cache: &Cache,
            guild: Id<GuildMarker>,
            member_bytes: AlignedVec<8>,
            user: &User,
        ) -> Result<CacheChange> {
            let user_bytes = {
                let mut serializer = AlignedVec::<8>::new();
                let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
                let with = With::<_, CachedUser>::cast(user);
                rkyv::api::serialize_using(with, strategy).wrap_err("Failed to serialize user")?;

                serializer
            };

            let mut conn = cache.connection().await?;

            let items = &[
                (RedisKey::member(guild, user.id), member_bytes.as_slice()),
                (RedisKey::user(user.id), user_bytes.as_slice()),
            ];

            conn.mset::<_, _, ()>(items)
                .await
                .wrap_err("Failed to store member or user bytes")?;

            let guild_key = RedisKey::guild_members(guild);

            conn.sadd::<_, _, ()>(guild_key, user.id.get())
                .await
                .wrap_err("Failed to add user as guild member")?;

            let added: isize = conn
                .sadd(RedisKey::users(), user.id.get())
                .await
                .wrap_err("Failed to add user as user id")?;

            Ok(CacheChange {
                users: added,
                ..Default::default()
            })
        }

        let member_bytes = rkyv::util::with_arena(|arena| {
            let mut serializer = Serializer::new(AlignedVec::<8>::new(), arena.acquire(), ());
            let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
            let with = With::<_, CachedMember>::cast(member);
            rkyv::api::serialize_using(with, strategy).wrap_err("Failed to serialize member")?;

            Ok::<_, Report>(serializer.into_writer())
        })?;

        inner(self, guild, member_bytes, user).await
    }

    pub(crate) async fn cache_members(
        &self,
        guild: Id<GuildMarker>,
        members: &[TwMember],
    ) -> Result<CacheChange> {
        if members.is_empty() {
            return Ok(CacheChange::default());
        }

        let (zipped_members, users) = rkyv::util::with_arena(|arena| {
            let mut serializer = Serializer::new(AlignedVec::<8>::new(), arena.acquire(), ());

            members
                .iter()
                .map(move |member| {
                    let user_id = member.user.id;

                    let user = {
                        let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer.writer);
                        let with = With::<_, CachedUser>::cast(&member.user);

                        rkyv::api::serialize_using(with, strategy)
                            .wrap_err("Failed to serialize user")
                            .map(|_| {
                                let bytes = serializer.writer.as_slice().to_vec();
                                serializer.writer.clear();

                                (RedisKey::from(&member.user), bytes)
                            })
                    };

                    let member = {
                        let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
                        let with = With::<_, CachedMember>::cast(member);

                        rkyv::api::serialize_using(with, strategy)
                            .wrap_err("Failed to serialize member")
                            .map(|_| {
                                let bytes = serializer.writer.as_slice().to_vec();
                                serializer.writer.clear();
                                let key = RedisKey::member(guild, member.user.id);

                                (key, bytes)
                            })
                    };

                    match (member, user) {
                        (Ok(member), Ok(user)) => Ok(((member, user_id.get()), user)),
                        (Err(e), _) | (_, Err(e)) => Err(e),
                    }
                })
                .collect::<Result<Zipped<Zipped<Vec<_>, Vec<_>>, Vec<_>>>>()
        })?
        .into_parts();

        let (members, member_ids) = zipped_members.into_parts();

        let mut conn = self.connection().await?;

        conn.mset::<_, _, ()>(&members)
            .await
            .wrap_err("Failed to store members bytes")?;

        conn.mset::<_, _, ()>(&users)
            .await
            .wrap_err("Failed to store users bytes")?;

        let guild_key = RedisKey::guild_members(guild);

        conn.sadd::<_, _, ()>(guild_key, &member_ids)
            .await
            .wrap_err("Failed to add users as guild members")?;

        let added: isize = conn
            .sadd(RedisKey::users(), &member_ids)
            .await
            .wrap_err("Failed to add users as user ids")?;

        Ok(CacheChange {
            users: added,
            ..Default::default()
        })
    }

    pub(crate) async fn cache_partial_guild(&self, guild: &PartialGuild) -> Result<CacheChange> {
        let mut change = self.cache_roles(guild.id, &guild.roles).await?;

        let mut conn = self.connection().await?;

        let bytes = {
            let mut serializer = AlignedVec::<8>::new();
            let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
            let with = With::<_, CachedGuild>::cast(guild);
            rkyv::api::serialize_using(with, strategy).wrap_err("Failed to serialize guild")?;

            serializer
        };

        let key = RedisKey::guild(guild.id);

        conn.set::<_, _, ()>(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store guild bytes")?;

        let guilds_added: isize = conn
            .sadd(RedisKey::guilds(), guild.id.get())
            .await
            .wrap_err("Failed to add guild as guild id")?;

        let unavailable_guilds_removed: isize = conn
            .srem(RedisKey::unavailable_guilds(), guild.id.get())
            .await
            .wrap_err("Failed to remove guild as unavailable guild id")?;

        change.guilds += guilds_added;
        change.unavailable_guilds -= unavailable_guilds_removed;

        Ok(change)
    }

    pub(crate) async fn cache_partial_member(
        &self,
        guild_id: Id<GuildMarker>,
        member: &PartialMember,
        user: &User,
    ) -> Result<CacheChange> {
        self.cache_member_user(guild_id, member, user).await
    }

    pub(crate) async fn cache_role(
        &self,
        guild: Id<GuildMarker>,
        role: &Role,
    ) -> Result<CacheChange> {
        let bytes = {
            let mut serializer = AlignedVec::<8>::new();
            let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
            let with = With::<_, CachedRole>::cast(role);
            rkyv::api::serialize_using(with, strategy).wrap_err("Failed to serialize role")?;

            serializer
        };

        let mut conn = self.connection().await?;
        let key = RedisKey::role(guild, role.id);

        conn.set::<_, _, ()>(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store role bytes")?;

        let guild_key = RedisKey::guild_roles(guild);

        conn.sadd::<_, _, ()>(guild_key, role.id.get())
            .await
            .wrap_err("Failed to add role as guild role")?;

        let added: isize = conn
            .sadd(RedisKey::roles(), role.id.get())
            .await
            .wrap_err("Failed to add role as role id")?;

        Ok(CacheChange {
            roles: added,
            ..Default::default()
        })
    }

    pub(crate) async fn cache_roles<'r, I>(
        &self,
        guild: Id<GuildMarker>,
        roles: I,
    ) -> Result<CacheChange>
    where
        I: IntoIterator<Item = &'r Role>,
    {
        let (roles, role_ids) = roles
            .into_iter()
            .map(|role| {
                let bytes = {
                    let mut serializer = AlignedVec::<8>::new();
                    let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
                    let with = With::<_, CachedRole>::cast(role);
                    rkyv::api::serialize_using(with, strategy)
                        .wrap_err("Failed to serialize role")?;

                    serializer
                };

                let key = RedisKey::role(guild, role.id);

                Ok::<_, Report>(((key, AlignedVecRedisArgs(bytes)), role.id.get()))
            })
            .collect::<Result<Zipped<Vec<_>, Vec<_>>, _>>()?
            .into_parts();

        if roles.is_empty() {
            return Ok(CacheChange::default());
        }

        let mut conn = self.connection().await?;

        conn.mset::<_, _, ()>(&roles)
            .await
            .wrap_err("Failed to store roles bytes")?;

        let guild_key = RedisKey::guild_roles(guild);

        conn.sadd::<_, _, ()>(guild_key, &role_ids)
            .await
            .wrap_err("Failed to add roles as guild roles")?;

        let added: isize = conn
            .sadd(RedisKey::roles(), &role_ids)
            .await
            .wrap_err("Failed to add roles as role ids")?;

        Ok(CacheChange {
            roles: added,
            ..Default::default()
        })
    }

    pub(crate) async fn cache_unavailable_guild(
        &self,
        guild: Id<GuildMarker>,
    ) -> Result<CacheChange> {
        let mut conn = self.connection().await?;

        let is_moved: bool = conn
            .smove(
                RedisKey::guilds(),
                RedisKey::unavailable_guilds(),
                guild.get(),
            )
            .await
            .wrap_err("Failed to move guild id")?;

        let change = if is_moved {
            conn.del::<_, ()>(RedisKey::guild(guild))
                .await
                .wrap_err("Failed to delete guild entry")?;

            let mut change = self
                .delete_guild_items(guild)
                .await
                .wrap_err("Failed to delete guild items")?;

            change.guilds -= 1;
            change.unavailable_guilds += 1;

            change
        } else {
            let added: isize = conn
                .sadd(RedisKey::unavailable_guilds(), guild.get())
                .await
                .wrap_err("Failed to add guild to unavailable guilds")?;

            CacheChange {
                unavailable_guilds: added,
                ..Default::default()
            }
        };

        Ok(change)
    }

    pub(crate) async fn cache_user(&self, user: &User) -> Result<CacheChange> {
        let mut conn = self.connection().await?;

        let bytes = {
            let mut serializer = AlignedVec::<8>::new();
            let strategy = Strategy::<_, BoxedError>::wrap(&mut serializer);
            let with = With::<_, CachedUser>::cast(user);
            rkyv::api::serialize_using(with, strategy).wrap_err("Failed to serialize user")?;

            serializer
        };

        let key = RedisKey::from(user);

        conn.set::<_, _, ()>(key, bytes.as_slice())
            .await
            .wrap_err("Failed to store user bytes")?;

        let added: isize = conn
            .sadd(RedisKey::users(), user.id.get())
            .await
            .wrap_err("Failed to add user as user id")?;

        Ok(CacheChange {
            users: added,
            ..Default::default()
        })
    }
}

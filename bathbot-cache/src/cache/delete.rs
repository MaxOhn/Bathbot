use std::iter;

use bb8_redis::{bb8::PooledConnection, redis::AsyncCommands, RedisConnectionManager};
use eyre::{Result, WrapErr};
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
    Id,
};

use crate::{key::RedisKey, model::CacheChange, Cache};

impl Cache {
    pub(crate) async fn delete_channel(
        &self,
        guild: Option<Id<GuildMarker>>,
        channel: Id<ChannelMarker>,
    ) -> Result<CacheChange> {
        let mut conn = self.connection().await?;

        conn.del::<_, ()>(RedisKey::channel(guild, channel))
            .await
            .wrap_err("Failed to delete channel entry")?;

        if let Some(guild) = guild {
            conn.srem::<_, _, ()>(RedisKey::guild_channels(guild), channel.get())
                .await
                .wrap_err("Failed to remove channel as guild channel")?;
        }

        let removed: isize = conn
            .srem(RedisKey::channels(), channel.get())
            .await
            .wrap_err("Failed to remove channel from channel ids")?;

        Ok(CacheChange {
            channels: -removed,
            ..Default::default()
        })
    }

    pub(crate) async fn delete_guild(&self, guild: Id<GuildMarker>) -> Result<CacheChange> {
        let mut conn = self.connection().await?;

        conn.del::<_, ()>(RedisKey::guild(guild))
            .await
            .wrap_err("Failed to delete guild entry")?;

        let removed: isize = conn
            .srem(RedisKey::guilds(), guild.get())
            .await
            .wrap_err("Failed to remove guild id entry")?;

        let mut change = self.delete_guild_items(guild).await?;

        change.guilds -= removed;

        Ok(change)
    }

    pub(crate) async fn delete_guild_items(&self, guild: Id<GuildMarker>) -> Result<CacheChange> {
        async fn remove_ids<G, I, K, C>(
            conn: &mut PooledConnection<'_, RedisConnectionManager>,
            guild: Id<GuildMarker>,
            guild_set_key_fn: G,
            id_set_key_fn: Option<I>,
            item_key_fn: K,
            change_fn: C,
        ) -> Result<()>
        where
            G: FnOnce(Id<GuildMarker>) -> RedisKey<'static>,
            I: FnOnce() -> RedisKey<'static>,
            K: Fn(Id<GuildMarker>, u64) -> RedisKey<'static>,
            C: FnOnce(isize),
        {
            let guild_key = (guild_set_key_fn)(guild);

            let guild_ids: Vec<u64> = conn
                .smembers(&guild_key)
                .await
                .wrap_err("Failed smembers")?;

            if let Some(id_set_key_fn) = id_set_key_fn.filter(|_| !guild_ids.is_empty()) {
                let removed: isize = conn
                    .srem((id_set_key_fn)(), &guild_ids)
                    .await
                    .wrap_err("Failed srem")?;
                change_fn(removed);
            }

            let redis_keys: Vec<_> = guild_ids
                .into_iter()
                .map(|id| (item_key_fn)(guild, id))
                .chain(iter::once(guild_key))
                .collect();

            conn.del::<_, ()>(&redis_keys)
                .await
                .wrap_err("Failed del")?;

            Ok(())
        }

        let mut change = CacheChange::default();
        let mut conn = self.connection().await?;

        remove_ids(
            &mut conn,
            guild,
            RedisKey::guild_channels,
            Some(RedisKey::channels),
            |guild, channel| RedisKey::channel(Some(guild), Id::new(channel)),
            |removed| change.channels -= removed,
        )
        .await
        .wrap_err("Failed to remove guild channels")?;

        remove_ids(
            &mut conn,
            guild,
            RedisKey::guild_members,
            None::<fn() -> RedisKey<'static>>,
            |guild, user| RedisKey::member(guild, Id::new(user)),
            |_| (),
        )
        .await
        .wrap_err("Failed to remove guild members")?;

        remove_ids(
            &mut conn,
            guild,
            RedisKey::guild_roles,
            Some(RedisKey::roles),
            |guild, role| RedisKey::role(guild, Id::new(role)),
            |removed| change.roles -= removed,
        )
        .await
        .wrap_err("Failed to remove guild roles")?;

        Ok(change)
    }

    pub(crate) async fn delete_member(
        &self,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    ) -> Result<CacheChange> {
        let mut conn = self.connection().await?;

        let key = RedisKey::member(guild, user);

        conn.del::<_, ()>(key)
            .await
            .wrap_err("Failed to delete member entry")?;

        conn.srem::<_, _, ()>(RedisKey::guild_members(guild), user.get())
            .await
            .wrap_err("Failed to remove guild member entry")?;

        // Note that in the case that the member has no more common guilds with us,
        // its user data _won't_ be deleted.
        // There's no stored structure in place that provides a way to remove
        // such user data but it shouldn't matter much anyway.

        Ok(CacheChange::default())
    }

    pub(crate) async fn delete_role(
        &self,
        guild: Id<GuildMarker>,
        role: Id<RoleMarker>,
    ) -> Result<CacheChange> {
        let mut conn = self.connection().await?;

        conn.del::<_, ()>(RedisKey::role(guild, role))
            .await
            .wrap_err("Failed to delete role entry")?;

        conn.srem::<_, _, ()>(RedisKey::guild_roles(guild), role.get())
            .await
            .wrap_err("Failed to remove role as guild role")?;

        let removed: isize = conn
            .srem(RedisKey::roles(), role.get())
            .await
            .wrap_err("Failed to remove role from role ids")?;

        Ok(CacheChange {
            roles: -removed,
            ..Default::default()
        })
    }
}

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

        conn.del(RedisKey::channel(guild, channel))
            .await
            .wrap_err("Failed to delete channel entry")?;

        if let Some(guild) = guild {
            conn.srem(RedisKey::guild_channels_key(guild), channel.get())
                .await
                .wrap_err("Failed to remove channel as guild channel")?;
        }

        let removed: isize = conn
            .srem(RedisKey::channel_ids_key(), channel.get())
            .await
            .wrap_err("Failed to remove channel from channel ids")?;

        Ok(CacheChange {
            channels: -removed,
            ..Default::default()
        })
    }

    pub(crate) async fn delete_guild(&self, guild: Id<GuildMarker>) -> Result<CacheChange> {
        let mut conn = self.connection().await?;

        conn.del(RedisKey::guild(guild))
            .await
            .wrap_err("Failed to delete guild entry")?;

        let removed: isize = conn
            .srem(RedisKey::guild_ids_key(), guild.get())
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
            guild_key_fn: G,
            ids_fn: Option<I>,
            redis_key_fn: K,
            change_fn: C,
        ) -> Result<()>
        where
            G: FnOnce(Id<GuildMarker>) -> String,
            I: FnOnce() -> &'static str,
            K: Fn(Id<GuildMarker>, u64) -> RedisKey,
            C: FnOnce(isize),
        {
            let guild_ids: Vec<u64> = conn.get_del(&(guild_key_fn)(guild)).await?;

            if let Some(ids_fn) = ids_fn {
                let removed: isize = conn.srem((ids_fn)(), &guild_ids).await?;
                change_fn(removed);
            }

            let redis_keys: Vec<_> = guild_ids
                .into_iter()
                .map(|id| (redis_key_fn)(guild, id))
                .collect();

            conn.del(&redis_keys).await?;

            Ok(())
        }

        let mut change = CacheChange::default();
        let mut conn = self.connection().await?;

        remove_ids(
            &mut conn,
            guild,
            RedisKey::guild_channels_key,
            Some(RedisKey::channel_ids_key),
            |guild, channel| RedisKey::channel(Some(guild), Id::new(channel)),
            |removed| change.channels -= removed,
        )
        .await
        .wrap_err("Failed to remove guild channels")?;

        remove_ids(
            &mut conn,
            guild,
            RedisKey::guild_members_key,
            None::<fn() -> &'static str>,
            |guild, user| RedisKey::member(guild, Id::new(user)),
            |_| (),
        )
        .await
        .wrap_err("Failed to remove guild members")?;

        remove_ids(
            &mut conn,
            guild,
            RedisKey::guild_roles_key,
            Some(RedisKey::role_ids_key),
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

        conn.del(key)
            .await
            .wrap_err("Failed to delete member entry")?;

        conn.srem(RedisKey::guild_members_key(guild), user.get())
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

        conn.del(RedisKey::role(guild, role))
            .await
            .wrap_err("Failed to delete role entry")?;

        conn.srem(RedisKey::guild_roles_key(guild), role.get())
            .await
            .wrap_err("Failed to remove role as guild role")?;

        let removed: isize = conn
            .srem(RedisKey::role_ids_key(), role.get())
            .await
            .wrap_err("Failed to remove role from role ids")?;

        Ok(CacheChange {
            roles: -removed,
            ..Default::default()
        })
    }
}

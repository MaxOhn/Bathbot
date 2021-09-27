use crate::{
    commands::osu::ProfileSize,
    database::{GuildConfig, UserConfig},
    BotResult, Database, Name,
};

use dashmap::DashMap;
use futures::stream::StreamExt;
use rosu_v2::prelude::GameMode;
use twilight_model::id::{GuildId, UserId};

impl Database {
    #[cold]
    pub async fn get_guilds(&self) -> BotResult<DashMap<GuildId, GuildConfig>> {
        let mut stream = sqlx::query!("SELECT * FROM guild_configs").fetch(&self.pool);
        let guilds = DashMap::with_capacity(10_000);

        while let Some(entry) = stream.next().await.transpose()? {
            let guild_id: i64 = entry.guild_id;
            let config = serde_json::from_value(entry.config)?;

            guilds.insert(GuildId(guild_id as u64), config);
        }

        Ok(guilds)
    }

    pub async fn insert_guild_config(
        &self,
        guild_id: GuildId,
        config: &GuildConfig,
    ) -> BotResult<()> {
        let query = sqlx::query!(
            "INSERT INTO guild_configs VALUES ($1,$2) 
            ON CONFLICT (guild_id) DO UPDATE SET config=$2",
            guild_id.0 as i64,
            serde_json::to_value(config)?
        );

        query.execute(&self.pool).await?;
        info!("Inserted GuildConfig for guild {} into DB", guild_id);

        Ok(())
    }

    pub async fn get_user_config(&self, user_id: UserId) -> BotResult<Option<UserConfig>> {
        let query = sqlx::query!(
            "SELECT * FROM user_config WHERE discord_id=$1",
            user_id.0 as i64
        );

        match query.fetch_optional(&self.pool).await? {
            Some(entry) => {
                let config = UserConfig {
                    embeds_maximized: entry.embeds_maximized,
                    mode: entry.mode.map(|mode| mode as u8).map(GameMode::from),
                    osu_username: entry.osu_username.map(Name::from),
                    profile_size: entry.profile_size.map(ProfileSize::from),
                    show_retries: entry.show_retries,
                    twitch_id: entry.twitch_id.map(|id| id as u64),
                };

                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    pub async fn get_user_config_by_osu(&self, username: &str) -> BotResult<Option<UserConfig>> {
        let query = sqlx::query!("SELECT * FROM user_config WHERE osu_username=$1", username);

        match query.fetch_optional(&self.pool).await? {
            Some(entry) => {
                let config = UserConfig {
                    embeds_maximized: entry.embeds_maximized,
                    mode: entry.mode.map(|mode| mode as u8).map(GameMode::from),
                    osu_username: entry.osu_username.map(Name::from),
                    profile_size: entry.profile_size.map(ProfileSize::from),
                    show_retries: entry.show_retries,
                    twitch_id: entry.twitch_id.map(|id| id as u64),
                };

                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    pub async fn insert_user_config(&self, user_id: UserId, config: &UserConfig) -> BotResult<()> {
        let query = sqlx::query!(
            "INSERT INTO user_config (discord_id,embeds_maximized,mode,osu_username,profile_size,show_retries,twitch_id)
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            ON CONFLICT (discord_id) DO UPDATE SET embeds_maximized=$2,mode=$3,osu_username=$4,profile_size=$5,show_retries=$6,twitch_id=$7",
            user_id.0 as i64,
            config.embeds_maximized,
            config.mode.map(|m| m as i16),
            config.osu_username.as_deref(),
            config.profile_size.map(|size| size as i16),
            config.show_retries,
            config.twitch_id.map(|id| id as i64)
        );

        query.execute(&self.pool).await?;
        debug!("Inserted UserConfig for user {} into DB", user_id);

        Ok(())
    }

    pub async fn update_user_config_osu(&self, old: &str, new: &str) -> BotResult<()> {
        let query = sqlx::query!(
            "UPDATE user_config SET osu_username=$1 WHERE osu_username=$2",
            new,
            old
        );

        query.execute(&self.pool).await?;
        debug!("Replaced osu_username `{}` with `{}` in DB", old, new);

        Ok(())
    }
}

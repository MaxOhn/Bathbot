use crate::{
    commands::osu::ProfileSize,
    database::{models::OsuData, GuildConfig, UserConfig},
    BotResult, Database,
};

use dashmap::DashMap;
use futures::stream::StreamExt;
use rosu_v2::prelude::{GameMode, User};
use twilight_model::id::{GuildId, UserId};

impl Database {
    #[cold]
    pub async fn get_guilds(&self) -> BotResult<DashMap<GuildId, GuildConfig>> {
        let mut stream = sqlx::query!("SELECT * FROM guild_configs").fetch(&self.pool);
        let guilds = DashMap::with_capacity(10_000);

        while let Some(entry) = stream.next().await.transpose()? {
            let config = GuildConfig {
                authorities: serde_cbor::from_slice(&entry.authorities)?,
                prefixes: serde_cbor::from_slice(&entry.prefixes)?,
                with_lyrics: entry.with_lyrics,
            };

            guilds.insert(GuildId(entry.guild_id as u64), config);
        }

        Ok(guilds)
    }

    pub async fn upsert_guild_config(
        &self,
        guild_id: GuildId,
        config: &GuildConfig,
    ) -> BotResult<()> {
        let query = sqlx::query!(
            "INSERT INTO guild_configs (guild_id,authorities,prefixes,with_lyrics)\
            VALUES ($1,$2,$3,$4) ON CONFLICT (guild_id) DO \
            UPDATE \
            SET authorities=$2,\
                prefixes=$3,\
                with_lyrics=$4",
            guild_id.0 as i64,
            serde_cbor::to_vec(&config.authorities)?,
            serde_cbor::to_vec(&config.prefixes)?,
            config.with_lyrics,
        );

        query.execute(&self.pool).await?;
        info!("Inserted GuildConfig for guild {} into DB", guild_id);

        Ok(())
    }

    pub async fn get_user_osu(&self, user_id: UserId) -> BotResult<Option<OsuData>> {
        let query = sqlx::query!(
            "SELECT user_id,username \
            FROM\
                (SELECT osu_id \
                FROM user_configs \
                WHERE discord_id=$1) AS config \
            JOIN osu_user_names AS names ON config.osu_id=names.user_id",
            user_id.0 as i64
        );

        match query.fetch_optional(&self.pool).await? {
            Some(entry) => {
                let osu = OsuData::User {
                    user_id: entry.user_id as u32,
                    username: entry.username.into(),
                };

                Ok(Some(osu))
            }
            None => Ok(None),
        }
    }

    pub async fn get_user_config(&self, user_id: UserId) -> BotResult<Option<UserConfig>> {
        let query = sqlx::query!(
            "SELECT * \
            FROM\
              (SELECT * \
               FROM user_configs \
               WHERE discord_id=$1) AS config \
            JOIN osu_user_names AS names ON config.osu_id=names.user_id",
            user_id.0 as i64
        );

        match query.fetch_optional(&self.pool).await? {
            Some(entry) => {
                let osu = OsuData::User {
                    user_id: entry.user_id as u32,
                    username: entry.username.into(),
                };

                let config = UserConfig {
                    embeds_maximized: entry.embeds_maximized,
                    mode: entry.mode.map(|mode| mode as u8).map(GameMode::from),
                    osu: Some(osu),
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
        let query = sqlx::query!(
            "SELECT * \
            FROM\
              (SELECT user_id \
               FROM osu_user_names \
               WHERE username=$1) AS user_ids \
            JOIN user_configs ON user_ids.user_id=user_configs.osu_id",
            username
        );

        match query.fetch_optional(&self.pool).await? {
            Some(entry) => {
                let osu = OsuData::User {
                    user_id: entry.user_id as u32,
                    username: username.into(),
                };

                let config = UserConfig {
                    embeds_maximized: entry.embeds_maximized,
                    mode: entry.mode.map(|mode| mode as u8).map(GameMode::from),
                    osu: Some(osu),
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
        if let Some(OsuData::User { user_id, username }) = &config.osu {
            let query = sqlx::query!(
                "INSERT INTO osu_user_names (user_id, username)
                VALUES ($1,$2) ON CONFLICT (user_id) DO
                UPDATE
                SET username=$2",
                *user_id as i32,
                username.as_str()
            );

            query.execute(&self.pool).await?;
        }

        let query = sqlx::query!(
            "INSERT INTO user_configs (\
                discord_id,\
                embeds_maximized,\
                mode,\
                osu_id,\
                profile_size,\
                show_retries,\
                twitch_id\
            )\
            VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (discord_id) DO \
            UPDATE \
            SET embeds_maximized=$2,\
                mode=$3,\
                osu_id=$4,\
                profile_size=$5,\
                show_retries=$6,\
                twitch_id=$7",
            user_id.0 as i64,
            config.embeds_maximized,
            config.mode.map(|m| m as i16),
            config
                .osu
                .as_ref()
                .and_then(OsuData::user_id)
                .map(|id| id as i32),
            config.profile_size.map(|size| size as i16),
            config.show_retries,
            config.twitch_id.map(|id| id as i64)
        );

        query.execute(&self.pool).await?;
        debug!("Inserted UserConfig for user {} into DB", user_id);

        Ok(())
    }

    pub async fn update_osu_name(&self, user: &User) -> BotResult<()> {
        let query = sqlx::query!(
            "INSERT INTO osu_user_names (user_id,username)\
            VALUES ($1,$2) ON CONFLICT (user_id) DO \
            UPDATE \
            SET username=$2",
            user.user_id as i32,
            user.username,
        );

        query.execute(&self.pool).await?;

        Ok(())
    }
}

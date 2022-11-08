use bathbot_psql::{
    model::configs::{OsuUserId, OsuUsername, ScoreSize, UserConfig},
    Database,
};
use eyre::{Result, WrapErr};
use rosu_v2::prelude::{GameMode, Username};
use twilight_model::id::{marker::UserMarker, Id};

#[derive(Copy, Clone)]
pub struct UserConfigManager<'d> {
    psql: &'d Database,
}

impl<'d> UserConfigManager<'d> {
    pub fn new(psql: &'d Database) -> Self {
        Self { psql }
    }

    pub async fn with_osu_id(self, user_id: Id<UserMarker>) -> Result<UserConfig<OsuUserId>> {
        let config_fut = self
            .psql
            .select_user_config_with_osu_id_by_discord_id(user_id);

        match config_fut.await.wrap_err("failed to get user config")? {
            Some(config) => Ok(config),
            None => {
                let config = UserConfig::default();

                self.psql
                    .upsert_user_config(user_id, &config)
                    .await
                    .wrap_err("failed to insert default user config")
                    .map(|_| config)
            }
        }
    }

    pub async fn with_osu_name(self, user_id: Id<UserMarker>) -> Result<UserConfig<OsuUsername>> {
        let config_fut = self
            .psql
            .select_user_config_with_osu_name_by_discord_id(user_id);

        match config_fut.await.wrap_err("failed to get user config")? {
            Some(config) => Ok(config),
            None => self
                .psql
                .upsert_user_config(user_id, &UserConfig::default())
                .await
                .wrap_err("failed to insert default user config")
                .map(|_| UserConfig::default()),
        }
    }

    pub async fn mode(self, user_id: Id<UserMarker>) -> Result<Option<GameMode>> {
        self.psql
            .select_user_mode(user_id)
            .await
            .wrap_err("failed to get user mode from DB")
    }

    pub async fn osu_id(self, user_id: Id<UserMarker>) -> Result<Option<u32>> {
        self.psql
            .select_osu_id_by_discord_id(user_id)
            .await
            .wrap_err("failed to get user id from DB")
    }

    pub async fn osu_name(self, user_id: Id<UserMarker>) -> Result<Option<Username>> {
        self.psql
            .select_osu_name_by_discord_id(user_id)
            .await
            .wrap_err("failed to get username from DB")
    }

    pub async fn discord_from_osu_id(self, user_id: u32) -> Result<Option<Id<UserMarker>>> {
        self.psql
            .select_user_discord_id_by_osu_id(user_id)
            .await
            .wrap_err("failed to get discord id from osu id")
    }

    pub async fn score_size(self, user_id: Id<UserMarker>) -> Result<Option<ScoreSize>> {
        self.psql
            .select_user_score_size(user_id)
            .await
            .wrap_err("failed to get user score size from DB")
    }

    pub async fn store(
        self,
        user_id: Id<UserMarker>,
        config: &UserConfig<OsuUserId>,
    ) -> Result<()> {
        self.psql
            .upsert_user_config(user_id, config)
            .await
            .wrap_err("failed to store user config")
    }
}

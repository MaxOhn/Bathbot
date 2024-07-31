use bathbot_model::command_fields::ScoreEmbedSettings;
use bathbot_psql::{
    model::configs::{OsuUserId, SkinEntry, UserConfig},
    Database,
};
use bathbot_util::CowUtils;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::{GameMode, Username};
use twilight_model::id::{marker::UserMarker, Id};

use crate::core::Context;

#[derive(Copy, Clone)]
pub struct UserConfigManager {
    psql: &'static Database,
}

impl UserConfigManager {
    pub fn new() -> Self {
        Self {
            psql: Context::psql(),
        }
    }

    pub async fn with_osu_id(self, user_id: Id<UserMarker>) -> Result<UserConfig<OsuUserId>> {
        let config_fut = self
            .psql
            .select_user_config_with_osu_id_by_discord_id(user_id);

        match config_fut.await.wrap_err("Failed to get user config")? {
            Some(config) => Ok(config),
            None => {
                let config = UserConfig::default();

                self.psql
                    .upsert_user_config(user_id, &config)
                    .await
                    .wrap_err("Failed to insert default user config")
                    .map(|_| config)
            }
        }
    }

    pub async fn mode(self, user_id: Id<UserMarker>) -> Result<Option<GameMode>> {
        self.psql
            .select_user_mode(user_id)
            .await
            .wrap_err("Failed to get user mode from DB")
    }

    pub async fn osu_id(self, user_id: Id<UserMarker>) -> Result<Option<u32>> {
        self.psql
            .select_osu_id_by_discord_id(user_id)
            .await
            .wrap_err("Failed to get user id from DB")
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

    pub async fn skin(self, user_id: Id<UserMarker>) -> Result<Option<String>> {
        self.psql
            .select_skin_url(user_id)
            .await
            .wrap_err("failed to get skin url")
    }

    pub async fn skin_from_osu_id(self, user_id: u32) -> Result<Option<String>> {
        self.psql
            .select_skin_url_by_osu_id(user_id)
            .await
            .wrap_err("failed to get skin url by user id")
    }

    pub async fn skin_from_osu_name(self, username: &str) -> Result<Option<String>> {
        let username = username.cow_replace('_', r"\_");

        self.psql
            .select_skin_url_by_osu_name(username.as_ref())
            .await
            .wrap_err("failed to get skin url by username")
    }

    pub async fn all_skins(self) -> Result<Vec<SkinEntry>> {
        self.psql
            .select_all_skins()
            .await
            .wrap_err("Failed to select all skins")
    }

    pub async fn update_skin(self, user_id: Id<UserMarker>, skin_url: Option<&str>) -> Result<()> {
        self.psql
            .update_skin_url(user_id, skin_url)
            .await
            .wrap_err("failed to update skin")
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

    pub async fn store_score_embed_settings(
        self,
        user_id: Id<UserMarker>,
        settings: &ScoreEmbedSettings,
    ) -> Result<()> {
        self.psql
            .update_score_embed_settings(user_id, settings)
            .await
            .wrap_err("Failed to store score embed settings")
    }
}

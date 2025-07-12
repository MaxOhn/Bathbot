use std::fmt::Write;

use bathbot_model::{RespektiveUser, RespektiveUsers};
use eyre::{Result, WrapErr};
use rosu_v2::model::GameMode;

use crate::{Client, site::Site};

impl Client {
    pub async fn get_respektive_users(
        &self,
        user_ids: impl IntoIterator<Item = u32>,
        mode: GameMode,
    ) -> Result<RespektiveUsers> {
        let mut url = "https://score.respektive.pw/u/".to_owned();

        let mut user_ids = user_ids.into_iter();

        let user_id = user_ids.next().expect("require at least one user id");
        let _ = write!(url, "{user_id}");

        for user_id in user_ids {
            let _ = write!(url, ",{user_id}");
        }

        let _ = write!(url, "?m={}", mode as u8);

        let bytes = self.make_get_request(url, Site::Respektive).await?;

        let users: Vec<RespektiveUser> = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize respektive user: {body}")
        })?;

        Ok(users.into())
    }

    pub async fn get_respektive_rank(
        &self,
        rank: u32,
        mode: GameMode,
    ) -> Result<Option<RespektiveUser>> {
        let url = format!("https://score.respektive.pw/rank/{rank}?m={}", mode as u8);
        let bytes = self.make_get_request(url, Site::Respektive).await?;

        let mut users: Vec<RespektiveUser> =
            serde_json::from_slice(&bytes).wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize respektive rank: {body}")
            })?;

        Ok(users
            .pop()
            .filter(|user| user.rank.is_some() || user.rank_highest.is_some()))
    }
}

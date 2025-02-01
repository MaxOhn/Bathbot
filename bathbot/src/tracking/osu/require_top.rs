use bathbot_psql::model::osu::DbTrackedOsuUserInChannel;
use eyre::{Result, WrapErr};
use rosu_v2::{model::GameMode, prelude::Score};

use super::OsuTracking;
use crate::core::Context;

/// If [`OsuTracking::add_user`] was missing the user's 100th score's pp value,
/// callers must request the top scores and use [`RequireTopScores::callback`].
pub struct RequireTopScores {
    entry: DbTrackedOsuUserInChannel,
    channel_id: u64,
    called_back: bool,
}

impl RequireTopScores {
    pub const fn new(entry: DbTrackedOsuUserInChannel, channel_id: u64) -> Self {
        Self {
            entry,
            channel_id,
            called_back: false,
        }
    }

    pub const fn user_id(&self) -> u32 {
        self.entry.user_id as u32
    }

    pub const fn mode(&self) -> GameMode {
        match self.entry.gamemode {
            0 => GameMode::Osu,
            1 => GameMode::Taiko,
            2 => GameMode::Catch,
            3 => GameMode::Mania,
            _ => GameMode::Osu,
        }
    }

    /// `top_scores` needs to be the user's top scores for the [`GameMode`].
    pub async fn callback(mut self, top_scores: &[Score]) -> Result<()> {
        let user_id = self.user_id();
        let mode = self.mode();

        let entry_opt = OsuTracking::users()
            .pin()
            .get(&user_id)
            .map(|user| user.get_unchecked(mode));

        if let Some(entry) = entry_opt {
            entry.insert_last_pp(user_id, mode, top_scores).await;
        }

        Context::psql()
            .upsert_tracked_osu_user(&self.entry, self.channel_id)
            .await
            .wrap_err("Failed to upsert tracked osu user")?;

        self.called_back = true;

        Ok(())
    }
}

impl Drop for RequireTopScores {
    fn drop(&mut self) {
        if !self.called_back {
            panic!("must use `RequireTopScores::callback`");
        }
    }
}

use bathbot_macros::pagination;
use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::osu::DbScores;
use bathbot_util::IntHasher;
use rosu_v2::prelude::GameMode;
use twilight_model::channel::message::Embed;

use super::Pages;
use crate::{
    commands::osu::ScoresOrder,
    embeds::{EmbedData, UserScoresEmbed},
    manager::redis::RedisData,
};

#[pagination(per_page = 10, entries = "scores")]
pub struct UserScoresPagination {
    scores: DbScores<IntHasher>,
    user: RedisData<User>,
    mode: Option<GameMode>,
    sort: ScoresOrder,
}

impl UserScoresPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        UserScoresEmbed::new(&self.scores, &self.user, self.mode, self.sort, pages).build()
    }
}

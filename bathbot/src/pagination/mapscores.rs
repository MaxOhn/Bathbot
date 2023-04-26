use bathbot_macros::pagination;
use bathbot_model::twilight_model::util::ImageHash;
use bathbot_psql::model::osu::DbScores;
use bathbot_util::IntHasher;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    channel::message::Embed,
    id::{marker::GuildMarker, Id},
};

use super::Pages;
use crate::{
    commands::osu::ScoresOrder,
    embeds::{EmbedData, MapScoresEmbed},
};

#[pagination(per_page = 10, entries = "scores")]
pub struct MapScoresPagination {
    scores: DbScores<IntHasher>,
    mode: Option<GameMode>,
    sort: ScoresOrder,
    guild_icon: Option<(Id<GuildMarker>, ImageHash)>,
}

impl MapScoresPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        MapScoresEmbed::new(&self.scores, self.mode, self.sort, self.guild_icon, pages).build()
    }
}

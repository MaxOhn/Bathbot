use std::collections::HashMap;

use bathbot_macros::pagination;
use bathbot_model::ScraperScore;
use bathbot_util::IntHasher;
use rosu_pp::DifficultyAttributes;
use rosu_v2::prelude::Username;
use twilight_model::channel::message::embed::Embed;

use super::Pages;
use crate::{
    core::Context,
    embeds::{EmbedData, LeaderboardEmbed},
    manager::OsuMap,
};

#[pagination(per_page = 10, entries = "scores")]
pub struct LeaderboardPagination {
    map: OsuMap,
    scores: Vec<ScraperScore>,
    stars: f32,
    max_combo: u32,
    attr_map: HashMap<u32, (DifficultyAttributes, f32), IntHasher>,
    author_name: Option<Username>,
    first_place_icon: Option<String>,
}

impl LeaderboardPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        let end_idx = self.scores.len().min(pages.index() + pages.per_page());
        let scores = &self.scores[pages.index()..end_idx];

        let embed_fut = LeaderboardEmbed::new(
            self.author_name.as_deref(),
            &self.map,
            self.stars,
            self.max_combo,
            &mut self.attr_map,
            Some(scores),
            &self.first_place_icon,
            pages,
            ctx,
        );

        embed_fut.await.build()
    }
}

use bathbot_macros::pagination;
use rosu_v2::prelude::Score;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::CompareEntry,
    embeds::{EmbedData, ScoresEmbed},
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
};

use super::Pages;

#[pagination(per_page = 10, entries = "entries")]
pub struct ScoresPagination {
    user: RedisData<User>,
    map: OsuMap,
    entries: Vec<CompareEntry>,
    pinned: Vec<Score>,
    personal: Vec<Score>,
    global_idx: Option<(usize, usize)>,
    pp_idx: usize,
}

impl ScoresPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.entries.len().min(pages.index + pages.per_page);
        let entries = &self.entries[pages.index..end_idx];

        let global_idx = self
            .global_idx
            .filter(|(idx, _)| (pages.index..pages.index + pages.per_page).contains(idx))
            .map(|(score_idx, map_idx)| {
                let factor = score_idx / pages.per_page;
                let new_idx = score_idx - factor * pages.per_page;

                (new_idx, map_idx)
            });

        let embed = ScoresEmbed::new(
            &self.user,
            &self.map,
            entries,
            &self.pinned,
            &self.personal,
            global_idx,
            self.pp_idx,
            pages,
        );

        embed.build()
    }
}

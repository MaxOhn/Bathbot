use bathbot_macros::pagination;
use rosu_v2::prelude::Score;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{CompareEntry, GlobalIndex},
    embeds::{EmbedData, MessageOrigin, ScoresEmbed},
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
    global_idx: Option<GlobalIndex>,
    pp_idx: usize,
    origin: MessageOrigin,
}

impl ScoresPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let global_idx = self
            .global_idx
            .as_ref()
            .filter(|global| {
                (pages.index()..pages.index() + pages.per_page()).contains(&global.idx_in_entries)
            })
            .map(|global| {
                let factor = global.idx_in_entries / pages.per_page();
                let new_idx = global.idx_in_entries - factor * pages.per_page();

                (new_idx, global.idx_in_map_lb)
            });

        let embed = ScoresEmbed::new(
            &self.user,
            &self.map,
            entries,
            &self.pinned,
            &self.personal,
            global_idx,
            self.pp_idx,
            &self.origin,
            pages,
        );

        embed.build()
    }
}

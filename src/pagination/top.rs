use std::collections::HashMap;

use bathbot_psql::model::configs::MinimizedPp;
use command_macros::pagination;
use eyre::Result;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{TopEntry, TopScoreOrder},
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    embeds::{CondensedTopEmbed, EmbedData, TopEmbed, TopSingleEmbed},
    manager::redis::{osu::User, RedisData},
    util::hasher::IntHasher,
};

use super::Pages;

#[pagination(per_page = 5, entries = "entries")]
pub struct TopPagination {
    user: RedisData<User>,
    entries: Vec<TopEntry>,
    sort_by: TopScoreOrder,
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool), IntHasher>,
}

impl TopPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.entries.len().min(pages.index + pages.per_page);
        let scores = &self.entries[pages.index..end_idx];

        TopEmbed::new(&self.user, scores, self.sort_by, &self.farm, pages).build()
    }
}

#[pagination(per_page = 10, entries = "entries")]
pub struct TopCondensedPagination {
    user: RedisData<User>,
    entries: Vec<TopEntry>,
    sort_by: TopScoreOrder,
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool), IntHasher>,
}

impl TopCondensedPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.entries.len().min(pages.index + pages.per_page);
        let scores = &self.entries[pages.index..end_idx];

        CondensedTopEmbed::new(&self.user, scores, self.sort_by, &self.farm, pages).build()
    }
}

#[pagination(per_page = 1, entries = "entries")]
pub struct TopSinglePagination {
    user: RedisData<User>,
    entries: Vec<TopEntry>,
    minimized_pp: MinimizedPp,
}

impl TopSinglePagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let entry = &self.entries[pages.index];
        let personal_idx = Some(entry.original_idx);

        let embed_fut = TopSingleEmbed::new(
            &self.user,
            entry,
            personal_idx,
            None,
            self.minimized_pp,
            ctx,
        );

        Ok(embed_fut.await.into_minimized())
    }
}

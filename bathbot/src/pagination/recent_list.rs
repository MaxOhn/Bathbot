use std::collections::HashMap;

use bathbot_macros::pagination;
use bathbot_util::IntHasher;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::RecentListEntry,
    embeds::{EmbedData, RecentListEmbed},
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
};

use super::Pages;

#[pagination(per_page = 10, entries = "entries")]
pub struct RecentListPagination {
    user: RedisData<User>,
    entries: Vec<RecentListEntry>,
    maps: HashMap<u32, OsuMap, IntHasher>,
}

impl RecentListPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        RecentListEmbed::new(&self.user, entries, &self.maps, pages).build()
    }
}

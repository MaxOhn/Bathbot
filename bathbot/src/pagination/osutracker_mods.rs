use bathbot_macros::pagination;
use twilight_model::channel::embed::Embed;

use crate::{
    custom_client::OsuTrackerModsEntry,
    embeds::{EmbedData, OsuTrackerModsEmbed},
};

use super::Pages;

#[pagination(per_page = 20, entries = "entries")]
pub struct OsuTrackerModsPagination {
    entries: Vec<OsuTrackerModsEntry>,
}

impl OsuTrackerModsPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index;
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page)];

        OsuTrackerModsEmbed::new(entries, pages).build()
    }
}

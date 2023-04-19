use bathbot_macros::pagination;
use bathbot_model::OsuTrackerModsEntry;
use twilight_model::channel::message::embed::Embed;

use super::Pages;
use crate::embeds::{EmbedData, OsuTrackerModsEmbed};

#[pagination(per_page = 20, entries = "entries")]
pub struct OsuTrackerModsPagination {
    entries: Box<[OsuTrackerModsEntry]>,
}

impl OsuTrackerModsPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index();
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page())];

        OsuTrackerModsEmbed::new(entries, pages).build()
    }
}

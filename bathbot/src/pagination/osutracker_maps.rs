use bathbot_macros::pagination;
use bathbot_model::OsuTrackerPpEntry;
use twilight_model::channel::message::embed::Embed;

use super::Pages;
use crate::embeds::{EmbedData, OsuTrackerMapsEmbed};

#[pagination(per_page = 10, entries = "entries")]
pub struct OsuTrackerMapsPagination {
    pp: u32,
    entries: Vec<OsuTrackerPpEntry>,
}

impl OsuTrackerMapsPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index();
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page())];

        OsuTrackerMapsEmbed::new(self.pp, entries, pages).build()
    }
}

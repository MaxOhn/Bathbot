use twilight_model::channel::Message;

use crate::{custom_client::OsuTrackerModsEntry, embeds::OsuTrackerModsEmbed, BotResult};

use super::{Pages, Pagination};

pub struct OsuTrackerModsPagination {
    msg: Message,
    pages: Pages,
    entries: Vec<OsuTrackerModsEntry>,
}

impl OsuTrackerModsPagination {
    pub fn new(msg: Message, entries: Vec<OsuTrackerModsEntry>) -> Self {
        Self {
            pages: Pages::new(20, entries.len()),
            msg,
            entries,
        }
    }
}

#[async_trait]
impl Pagination for OsuTrackerModsPagination {
    type PageData = OsuTrackerModsEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let index = self.pages.index;
        let entries = &self.entries[index..(index + 20).min(self.entries.len())];
        let page = self.page();
        let pages = self.pages.total_pages;
        let embed = OsuTrackerModsEmbed::new(entries, (page, pages));

        Ok(embed)
    }
}

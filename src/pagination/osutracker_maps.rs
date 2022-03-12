use twilight_model::channel::Message;

use crate::{custom_client::OsuTrackerPpEntry, embeds::OsuTrackerMapsEmbed, BotResult};

use super::{Pages, Pagination};

pub struct OsuTrackerMapsPagination {
    msg: Message,
    pages: Pages,
    pp: u32,
    entries: Vec<OsuTrackerPpEntry>,
}

impl OsuTrackerMapsPagination {
    pub fn new(msg: Message, pp: u32, entries: Vec<OsuTrackerPpEntry>) -> Self {
        Self {
            pages: Pages::new(10, entries.len()),
            msg,
            pp,
            entries,
        }
    }
}

#[async_trait]
impl Pagination for OsuTrackerMapsPagination {
    type PageData = OsuTrackerMapsEmbed;

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
        let entries = &self.entries[index..(index + 10).min(self.entries.len())];
        let page = self.page();
        let pages = self.pages.total_pages;
        let embed = OsuTrackerMapsEmbed::new(self.pp, entries, (page, pages));

        Ok(embed)
    }
}

use super::{Pages, Pagination};
use crate::{custom_client::OsekaiRarityEntry, embeds::MedalRarityEmbed, BotResult};

use twilight_model::channel::Message;

pub struct MedalRarityPagination {
    msg: Message,
    pages: Pages,
    ranking: Vec<OsekaiRarityEntry>,
}

impl MedalRarityPagination {
    pub fn new(msg: Message, ranking: Vec<OsekaiRarityEntry>) -> Self {
        Self {
            msg,
            pages: Pages::new(10, ranking.len()),
            ranking,
        }
    }
}

#[async_trait]
impl Pagination for MedalRarityPagination {
    type PageData = MedalRarityEmbed;

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
        let page = self.page();
        let idx = (page - 1) * self.pages.per_page;
        let limit = self.ranking.len().min(idx + self.pages.per_page);

        Ok(MedalRarityEmbed::new(
            &self.ranking[idx..limit],
            self.pages.index,
            (page, self.pages.total_pages),
        ))
    }
}

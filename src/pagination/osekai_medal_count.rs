use super::{Pages, Pagination, ReactionVec};
use crate::{custom_client::OsekaiUserEntry, embeds::MedalCountEmbed, BotResult};

use command_macros::BasePagination;
use twilight_model::channel::Message;

#[derive(BasePagination)]
#[pagination(single_step = 10, multi_step = 100)]
pub struct MedalCountPagination {
    msg: Message,
    pages: Pages,
    ranking: Vec<OsekaiUserEntry>,
    author_idx: Option<usize>,
}

impl MedalCountPagination {
    pub fn new(msg: Message, ranking: Vec<OsekaiUserEntry>, author_idx: Option<usize>) -> Self {
        Self {
            msg,
            pages: Pages::new(10, ranking.len()),
            ranking,
            author_idx,
        }
    }
}

#[async_trait]
impl Pagination for MedalCountPagination {
    type PageData = MedalCountEmbed;

    fn reactions() -> ReactionVec {
        Self::arrow_reactions_full()
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let page = self.page();
        let idx = (page - 1) * self.pages.per_page;
        let limit = self.ranking.len().min(idx + self.pages.per_page);

        Ok(MedalCountEmbed::new(
            &self.ranking[idx..limit],
            self.pages.index,
            self.author_idx,
            (page, self.pages.total_pages),
        ))
    }
}

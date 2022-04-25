use command_macros::BasePagination;
use twilight_model::channel::Message;

use crate::{
    commands::osu::{OsuTrackerCountryDetailsCompact, ScoreOrder},
    custom_client::OsuTrackerCountryScore,
    embeds::OsuTrackerCountryTopEmbed,
    BotResult,
};

use super::{Pages, Pagination};

#[derive(BasePagination)]
#[pagination(single_step = 10)]
pub struct OsuTrackerCountryTopPagination {
    msg: Message,
    pages: Pages,
    details: OsuTrackerCountryDetailsCompact,
    scores: Vec<(OsuTrackerCountryScore, usize)>,
    sort_by: ScoreOrder,
}

impl OsuTrackerCountryTopPagination {
    pub fn new(
        msg: Message,
        details: OsuTrackerCountryDetailsCompact,
        scores: Vec<(OsuTrackerCountryScore, usize)>,
        sort_by: ScoreOrder,
    ) -> Self {
        Self {
            pages: Pages::new(10, scores.len()),
            msg,
            details,
            scores,
            sort_by,
        }
    }
}

#[async_trait]
impl Pagination for OsuTrackerCountryTopPagination {
    type PageData = OsuTrackerCountryTopEmbed;

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let index = self.pages.index;
        let scores = &self.scores[index..(index + 10).min(self.scores.len())];
        let page = self.page();
        let pages = self.pages.total_pages;
        let embed =
            OsuTrackerCountryTopEmbed::new(&self.details, scores, self.sort_by, (page, pages));

        Ok(embed)
    }
}

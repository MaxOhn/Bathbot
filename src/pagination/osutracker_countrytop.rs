use twilight_model::channel::Message;

use crate::{
    commands::osu::OsuTrackerCountryDetailsCompact, custom_client::OsuTrackerCountryScore,
    embeds::OsuTrackerCountryTopEmbed, BotResult,
};

use super::{Pages, Pagination};

pub struct OsuTrackerCountryTopPagination {
    msg: Message,
    pages: Pages,
    details: OsuTrackerCountryDetailsCompact,
    scores: Vec<OsuTrackerCountryScore>,
}

impl OsuTrackerCountryTopPagination {
    pub fn new(
        msg: Message,
        details: OsuTrackerCountryDetailsCompact,
        scores: Vec<OsuTrackerCountryScore>,
    ) -> Self {
        Self {
            pages: Pages::new(10, scores.len()),
            msg,
            details,
            scores,
        }
    }
}

#[async_trait]
impl Pagination for OsuTrackerCountryTopPagination {
    type PageData = OsuTrackerCountryTopEmbed;

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
        let scores = &self.scores[index..(index + 10).min(self.scores.len())];
        let page = self.page();
        let pages = self.pages.total_pages;
        let embed = OsuTrackerCountryTopEmbed::new(&self.details, scores, (page, pages));

        Ok(embed)
    }
}

use super::{Pages, Pagination};

use crate::{
    commands::osu::MedalType, custom_client::OsuProfile, embeds::MedalsMissingEmbed, BotResult,
};

use async_trait::async_trait;
use twilight_model::channel::Message;

pub struct MedalsMissingPagination {
    msg: Message,
    pages: Pages,
    profile: OsuProfile,
    medals: Vec<MedalType>,
    medal_count: (usize, usize),
}

impl MedalsMissingPagination {
    pub fn new(
        msg: Message,
        profile: OsuProfile,
        medals: Vec<MedalType>,
        medal_count: (usize, usize),
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(15, medals.len()),
            profile,
            medals,
            medal_count,
        }
    }
}

#[async_trait]
impl Pagination for MedalsMissingPagination {
    type PageData = MedalsMissingEmbed;
    fn msg(&self) -> &Message {
        &self.msg
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let page = self.page();
        let idx = (page - 1) * 15;
        let limit = self.medals.len().min(idx + self.pages.per_page);
        Ok(MedalsMissingEmbed::new(
            &self.profile,
            &self.medals[idx..limit],
            self.medal_count,
            limit == self.medals.len(),
            (page, self.pages.total_pages),
        ))
    }
}

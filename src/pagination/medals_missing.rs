use super::{Pages, Pagination};

use crate::{commands::osu::MedalType, embeds::MedalsMissingEmbed, BotResult};

use rosu_v2::model::user::User;
use twilight_model::channel::Message;

pub struct MedalsMissingPagination {
    msg: Message,
    pages: Pages,
    user: User,
    medals: Vec<MedalType>,
    medal_count: (usize, usize),
}

impl MedalsMissingPagination {
    pub fn new(
        msg: Message,
        user: User,
        medals: Vec<MedalType>,
        medal_count: (usize, usize),
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(15, medals.len()),
            user,
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

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let page = self.page();
        let idx = (page - 1) * 15;
        let limit = self.medals.len().min(idx + self.pages.per_page);

        Ok(MedalsMissingEmbed::new(
            &self.user,
            &self.medals[idx..limit],
            self.medal_count,
            limit == self.medals.len(),
            (page, self.pages.total_pages),
        ))
    }
}

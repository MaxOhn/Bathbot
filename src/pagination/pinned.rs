use super::{Pages, Pagination};

use crate::{embeds::PinnedEmbed, BotResult};

use rosu_v2::prelude::{Score, User};
use twilight_model::channel::Message;

pub struct PinnedPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<Score>,
}

impl PinnedPagination {
    pub fn new(msg: Message, user: User, scores: Vec<Score>) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
        }
    }
}

#[async_trait]
impl Pagination for PinnedPagination {
    type PageData = PinnedEmbed;

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
        let scores = self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page);

        Ok(PinnedEmbed::new(&self.user, scores, (self.page(), self.pages.total_pages)).await)
    }
}

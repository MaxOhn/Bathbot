use std::sync::Arc;

use super::{Pages, Pagination};

use crate::{commands::osu::ScoreOrder, core::Context, embeds::PinnedEmbed, BotResult};

use rosu_v2::prelude::{Score, User};
use twilight_model::channel::Message;

pub struct PinnedPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<Score>,
    sort_by: ScoreOrder,
}

impl PinnedPagination {
    pub fn new(
        msg: Message,
        user: User,
        scores: Vec<Score>,
        sort_by: ScoreOrder,
        ctx: Arc<Context>,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
            ctx,
            sort_by,
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

        let fut = PinnedEmbed::new(
            &self.user,
            scores,
            &self.ctx,
            self.sort_by,
            (self.page(), self.pages.total_pages),
        );

        Ok(fut.await)
    }
}

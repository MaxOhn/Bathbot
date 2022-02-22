use std::sync::Arc;

use rosu_v2::prelude::{Score, User};
use twilight_model::channel::Message;

use crate::{core::Context, embeds::TopEmbed, BotResult};

use super::{Pages, Pagination};

pub struct TopPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score)>,
}

impl TopPagination {
    pub fn new(msg: Message, user: User, scores: Vec<(usize, Score)>, ctx: Arc<Context>) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for TopPagination {
    type PageData = TopEmbed;

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
        let fut = TopEmbed::new(
            &self.user,
            self.scores
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            &self.ctx,
            (self.page(), self.pages.total_pages),
        );

        Ok(fut.await)
    }
}

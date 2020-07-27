use super::{Pages, Pagination};

use crate::{embeds::NoChokeEmbed, BotResult, Context};

use async_trait::async_trait;
use rosu::models::{Beatmap, Score, User};
use std::sync::Arc;
use twilight::model::{channel::Message, id::UserId};

pub struct NoChokePagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score, Score, Beatmap)>,
    unchoked_pp: f64,
    ctx: Arc<Context>,
}

impl NoChokePagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: Vec<(usize, Score, Score, Beatmap)>,
        unchoked_pp: f64,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(5, scores.len()),
            user,
            scores,
            unchoked_pp,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for NoChokePagination {
    type PageData = NoChokeEmbed;
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
        NoChokeEmbed::new(
            &self.ctx,
            &self.user,
            self.scores
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            self.unchoked_pp,
            (self.page(), self.pages.total_pages),
        )
        .await
    }
}

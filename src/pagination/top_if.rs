use super::{Pages, Pagination};

use crate::{embeds::TopIfEmbed, BotResult, Context};

use async_trait::async_trait;
use rosu::model::{Beatmap, GameMode, Score, User};
use std::sync::Arc;
use twilight_model::channel::Message;

pub struct TopIfPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score, Beatmap)>,
    mode: GameMode,
    adjusted_pp: f32,
    ctx: Arc<Context>,
}

impl TopIfPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: Vec<(usize, Score, Beatmap)>,
        mode: GameMode,
        adjusted_pp: f32,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
            mode,
            adjusted_pp,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for TopIfPagination {
    type PageData = TopIfEmbed;
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
        Ok(TopIfEmbed::new(
            &self.ctx,
            &self.user,
            self.scores
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            self.mode,
            self.adjusted_pp,
            (self.page(), self.pages.total_pages),
        )
        .await)
    }
}

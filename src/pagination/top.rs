use super::{Pages, Pagination};

use crate::{embeds::TopEmbed, BotResult, Context};

use async_trait::async_trait;
use rosu::models::{Beatmap, GameMode, Score, User};
use std::sync::Arc;
use twilight::model::{channel::Message, id::UserId};

pub struct TopPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score, Beatmap)>,
    mode: GameMode,
    ctx: Arc<Context>,
}

impl TopPagination {
    pub async fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: Vec<(usize, Score, Beatmap)>,
        mode: GameMode,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
            mode,
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
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        TopEmbed::new(
            &self.user,
            self.scores
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            self.mode,
            (self.page(), self.pages.total_pages),
            self.ctx.clone(),
        )
        .await
    }
}

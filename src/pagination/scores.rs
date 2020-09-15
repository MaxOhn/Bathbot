use super::{Pages, Pagination};

use crate::{embeds::ScoresEmbed, BotResult, Context};

use async_trait::async_trait;
use rosu::models::{Beatmap, Score, User};
use std::sync::Arc;
use twilight_model::channel::Message;

pub struct ScoresPagination {
    msg: Message,
    pages: Pages,
    user: User,
    map: Beatmap,
    scores: Vec<Score>,
    ctx: Arc<Context>,
}

impl ScoresPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        map: Beatmap,
        scores: Vec<Score>,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(10, scores.len()),
            user,
            map,
            scores,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for ScoresPagination {
    type PageData = ScoresEmbed;
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
        let scores = self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page);
        ScoresEmbed::new(&self.ctx, &self.user, &self.map, scores, self.pages.index).await
    }
}

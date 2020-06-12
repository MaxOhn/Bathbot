use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, Error};

use rosu::models::{Beatmap, Score, User};
use serenity::{
    async_trait,
    cache::Cache,
    client::Context,
    collector::ReactionCollector,
    model::{channel::Message, id::UserId},
};
use std::sync::Arc;

pub struct NoChokePagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    user: Box<User>,
    scores: Vec<(usize, Score, Score, Beatmap)>,
    unchoked_pp: f64,
    cache: Arc<Cache>,
}

impl NoChokePagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        user: User,
        scores: Vec<(usize, Score, Score, Beatmap)>,
        unchoked_pp: f64,
    ) -> Self {
        let collector = Self::create_collector(ctx, &msg, author, 90).await;
        let cache = Arc::clone(&ctx.cache);
        Self {
            msg,
            collector,
            pages: Pages::new(5, scores.len()),
            user: Box::new(user),
            scores,
            unchoked_pp,
            cache,
        }
    }
}

#[async_trait]
impl Pagination for NoChokePagination {
    type PageData = BasicEmbedData;
    fn msg(&mut self) -> &mut Message {
        &mut self.msg
    }
    fn collector(&mut self) -> &mut ReactionCollector {
        &mut self.collector
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        BasicEmbedData::create_nochoke(
            &*self.user,
            self.scores.iter().skip(self.index()).take(self.per_page()),
            self.unchoked_pp,
            (self.page(), self.total_pages()),
            &self.cache,
        )
        .await
    }
}

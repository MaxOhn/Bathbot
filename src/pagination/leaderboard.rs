use super::{create_collector, Pages, Pagination};

use crate::{embeds::LeaderboardEmbed, scraper::ScraperScore};

use failure::Error;
use rosu::models::Beatmap;
use serenity::{
    async_trait,
    cache::Cache,
    client::Context,
    collector::ReactionCollector,
    model::{channel::Message, id::UserId},
    prelude::{RwLock, TypeMap},
};
use std::sync::Arc;

pub struct LeaderboardPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    map: Beatmap,
    scores: Vec<ScraperScore>,
    author_name: Option<String>,
    first_place_icon: Option<String>,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
}

impl LeaderboardPagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        map: Beatmap,
        scores: Vec<ScraperScore>,
        author_name: Option<String>,
        first_place_icon: Option<String>,
    ) -> Self {
        let collector = create_collector(ctx, &msg, author, 60).await;
        let cache = Arc::clone(&ctx.cache);
        let data = Arc::clone(&ctx.data);
        Self {
            msg,
            collector,
            pages: Pages::new(10, scores.len()),
            map,
            scores,
            author_name,
            first_place_icon,
            cache,
            data,
        }
    }
}

#[async_trait]
impl Pagination for LeaderboardPagination {
    type PageData = LeaderboardEmbed;
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
        let scores = self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page);
        LeaderboardEmbed::new(
            &self.author_name.as_deref(),
            &self.map,
            Some(scores),
            &self.first_place_icon,
            self.pages.index,
            (&self.cache, &self.data),
        )
        .await
    }
}

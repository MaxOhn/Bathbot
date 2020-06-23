use super::{create_collector, Pages, Pagination};

use crate::{
    embeds::OsuStatsGlobalsEmbed,
    scraper::{OsuStatsParams, OsuStatsScore},
    Scraper,
};

use failure::Error;
use rosu::models::User;
use serenity::{
    async_trait,
    cache::Cache,
    collector::ReactionCollector,
    model::{channel::Message, id::UserId},
    prelude::{Context, RwLock, TypeMap},
};
use std::{collections::BTreeMap, iter::Extend, sync::Arc};

pub struct OsuStatsGlobalsPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    user: User,
    scores: BTreeMap<usize, OsuStatsScore>,
    total: usize,
    params: OsuStatsParams,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
}

impl OsuStatsGlobalsPagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        user: User,
        scores: BTreeMap<usize, OsuStatsScore>,
        total: usize,
        params: OsuStatsParams,
    ) -> Self {
        let collector = create_collector(ctx, &msg, author, 120).await;
        let cache = Arc::clone(&ctx.cache);
        let data = Arc::clone(&ctx.data);
        Self {
            pages: Pages::new(5, total),
            msg,
            collector,
            user,
            scores,
            total,
            params,
            cache,
            data,
        }
    }
}

#[async_trait]
impl Pagination for OsuStatsGlobalsPagination {
    type PageData = OsuStatsGlobalsEmbed;
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
    fn reactions() -> &'static [&'static str] {
        &["⏮️", "⏪", "◀️", "▶️", "⏩", "⏭️"]
    }
    fn single_step(&self) -> usize {
        5
    }
    fn multi_step(&self) -> usize {
        25
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        let entries = self.scores.range(self.pages.index..self.pages.index + 5);
        let count = entries.count();
        if count < 5 && self.total - self.pages.index > count {
            let osustats_page = (self.pages.index / 24) + 1;
            self.params.page(osustats_page);
            let data = self.data.read().await;
            let scraper = data.get::<Scraper>().unwrap();
            let (scores, _) = scraper.get_global_scores(&self.params).await?;
            let iter = scores
                .into_iter()
                .enumerate()
                .map(|(i, s)| ((osustats_page - 1) * 24 + i, s));
            self.scores.extend(iter);
        }
        OsuStatsGlobalsEmbed::new(
            &self.user,
            &self.scores,
            (self.page(), self.pages.total_pages),
            (&self.cache, &self.data),
        )
        .await
    }
}

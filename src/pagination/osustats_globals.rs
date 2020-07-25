use super::{Pages, Pagination};

use crate::{
    embeds::OsuStatsGlobalsEmbed,
    scraper::{OsuStatsParams, OsuStatsScore},
    BotResult, Context, Scraper,
};

use async_trait::async_trait;
use rosu::models::User;
use std::{collections::BTreeMap, iter::Extend, sync::Arc};
use twilight::model::{channel::Message, id::UserId};

pub struct OsuStatsGlobalsPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: BTreeMap<usize, OsuStatsScore>,
    total: usize,
    params: OsuStatsParams,
    ctx: Arc<Context>,
}

impl OsuStatsGlobalsPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: BTreeMap<usize, OsuStatsScore>,
        total: usize,
        params: OsuStatsParams,
    ) -> Self {
        Self {
            pages: Pages::new(5, total),
            msg,
            user,
            scores,
            total,
            params,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for OsuStatsGlobalsPagination {
    type PageData = OsuStatsGlobalsEmbed;
    fn msg(&self) -> &Message {
        &self.msg
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
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
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
            self.total,
            (self.page(), self.pages.total_pages),
            &self.ctx,
        )
        .await
    }
}

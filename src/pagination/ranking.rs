use super::{Pages, Pagination, ReactionVec};

use crate::{
    commands::osu::{RankingType, UserValue},
    embeds::RankingEmbed,
    BotResult, Context,
};

use async_trait::async_trait;
use rosu_v2::prelude::GameMode;
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};
use twilight_model::channel::Message;

type Users = BTreeMap<usize, (UserValue, String)>;

pub struct RankingPagination {
    msg: Message,
    pages: Pages,
    ctx: Arc<Context>,
    mode: GameMode,
    users: Users,
    title: Cow<'static, str>,
    url_type: &'static str,
    country_code: Option<String>,
    total: usize,
    ranking_type: RankingType,
}

impl RankingPagination {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        msg: Message,
        mode: GameMode,
        ctx: Arc<Context>,
        total: usize,
        users: Users,
        title: Cow<'static, str>,
        url_type: &'static str,
        country_code: Option<String>,
        ranking_type: RankingType,
    ) -> Self {
        Self {
            pages: Pages::new(20, total),
            msg,
            ctx,
            mode,
            users,
            title,
            url_type,
            country_code,
            total,
            ranking_type,
        }
    }

    async fn assure_present_users(&mut self, page: usize) -> BotResult<()> {
        let count = self
            .users
            .range(self.pages.index..self.pages.index + self.pages.per_page)
            .count();

        if count < self.pages.per_page && count < self.total - self.pages.index {
            let offset = page - 1;
            let page = page as u32;

            let ranking = match (self.ranking_type, &self.country_code) {
                (RankingType::Performance, Some(country)) => {
                    self.ctx
                        .osu()
                        .performance_rankings(self.mode)
                        .country(country)
                        .page(page)
                        .await?
                }
                (RankingType::Performance, None) => {
                    self.ctx
                        .osu()
                        .performance_rankings(self.mode)
                        .page(page)
                        .await?
                }
                (RankingType::Score, _) => {
                    self.ctx.osu().score_rankings(self.mode).page(page).await?
                }
            };

            let ranking_type = self.ranking_type;

            let iter = ranking
                .ranking
                .into_iter()
                .map(|user| match ranking_type {
                    RankingType::Performance => (
                        UserValue::Pp(user.statistics.as_ref().unwrap().pp.round() as u32),
                        user.username,
                    ),
                    RankingType::Score => (
                        UserValue::Score(user.statistics.as_ref().unwrap().ranked_score),
                        user.username,
                    ),
                })
                .enumerate()
                .map(|(i, tuple)| (offset * 50 + i, tuple));

            self.users.extend(iter);
        }

        Ok(())
    }
}

#[async_trait]
impl Pagination for RankingPagination {
    type PageData = RankingEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn reactions() -> ReactionVec {
        Self::arrow_reactions_full()
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    fn multi_step(&self) -> usize {
        self.pages.per_page * 10
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let idx = self.pages.index.saturating_sub(1);
        let mut page = ((idx - idx % 50) + 50) / 50;
        page += self.users.contains_key(&idx) as usize;

        self.assure_present_users(page).await?;

        // Handle edge cases like idx=140;total=151 where two pages have to be requested at once
        self.assure_present_users(page + 1).await?;

        Ok(RankingEmbed::new(
            self.mode,
            &self.users,
            &self.title,
            self.url_type,
            self.country_code.as_deref(),
            (self.page(), self.pages.total_pages),
        ))
    }
}

use super::{Pages, Pagination, ReactionVec};
use crate::{
    custom_client::{OsuStatsParams, OsuStatsScore},
    embeds::OsuStatsGlobalsEmbed,
    BotResult, Context,
};

use async_trait::async_trait;
use rosu_v2::model::user::User;
use std::{collections::BTreeMap, iter::Extend, sync::Arc};
use twilight_model::channel::Message;

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

    fn reactions() -> ReactionVec {
        Self::arrow_reactions_full()
    }

    fn single_step(&self) -> usize {
        5
    }

    fn multi_step(&self) -> usize {
        25
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let entries = self
            .scores
            .range(self.pages.index..self.pages.index + self.pages.per_page);

        let count = entries.count();

        if count < self.pages.per_page && self.total - self.pages.index > count {
            let osustats_page = (self.pages.index / 24) + 1;
            self.params.page = osustats_page;

            let (scores, _) = self
                .ctx
                .clients
                .custom
                .get_global_scores(&self.params)
                .await?;

            let iter = scores
                .into_iter()
                .enumerate()
                .map(|(i, s)| ((osustats_page - 1) * 24 + i, s));

            self.scores.extend(iter);
        }

        Ok(OsuStatsGlobalsEmbed::new(
            &self.user,
            &self.scores,
            self.total,
            (self.page(), self.pages.total_pages),
        )
        .await)
    }
}

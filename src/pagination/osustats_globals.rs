use std::{collections::BTreeMap, iter::Extend, sync::Arc};

use command_macros::BasePagination;
use rosu_v2::model::user::User;
use twilight_model::channel::Message;

use crate::{
    custom_client::{OsuStatsParams, OsuStatsScore},
    embeds::OsuStatsGlobalsEmbed,
    BotResult, Context,
};

use super::{Pages, Pagination, ReactionVec};

#[derive(BasePagination)]
#[pagination(single_step = 5, multi_step = 25)]
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

    fn reactions() -> ReactionVec {
        Self::arrow_reactions_full()
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let entries = self
            .scores
            .range(self.pages.index..self.pages.index + self.pages.per_page);

        let count = entries.count();

        if count < self.pages.per_page && self.total - self.pages.index > count {
            let osustats_page = (self.pages.index / 24) + 1;
            self.params.page = osustats_page;

            let (scores, _) = self.ctx.client().get_global_scores(&self.params).await?;

            let iter = scores
                .into_iter()
                .enumerate()
                .map(|(i, s)| ((osustats_page - 1) * 24 + i, s));

            self.scores.extend(iter);
        }

        let fut = OsuStatsGlobalsEmbed::new(
            &self.user,
            &self.scores,
            self.total,
            &self.ctx,
            (self.page(), self.pages.total_pages),
        );

        Ok(fut.await)
    }
}

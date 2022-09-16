use std::{collections::BTreeMap, iter::Extend};

use command_macros::pagination;
use eyre::{Result, WrapErr};
use rosu_v2::model::user::User;
use twilight_model::channel::embed::Embed;

use crate::{
    custom_client::{OsuStatsParams, OsuStatsScore},
    embeds::{EmbedData, OsuStatsGlobalsEmbed},
    Context,
};

use super::Pages;

#[pagination(per_page = 5, total = "total")]
pub struct OsuStatsGlobalsPagination {
    user: User,
    scores: BTreeMap<usize, OsuStatsScore>,
    total: usize,
    params: OsuStatsParams,
}

impl OsuStatsGlobalsPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let entries = self.scores.range(pages.index..pages.index + pages.per_page);
        let count = entries.count();

        if count < pages.per_page && self.total - pages.index > count {
            let osustats_page = (pages.index / 24) + 1;
            self.params.page = osustats_page;

            let (scores, _) = ctx
                .client()
                .get_global_scores(&self.params)
                .await
                .wrap_err("failed to get global scores")?;

            let iter = scores
                .into_iter()
                .enumerate()
                .map(|(i, s)| ((osustats_page - 1) * 24 + i, s));

            self.scores.extend(iter);
        }

        let embed_fut = OsuStatsGlobalsEmbed::new(&self.user, &self.scores, self.total, ctx, pages);

        Ok(embed_fut.await.build())
    }
}

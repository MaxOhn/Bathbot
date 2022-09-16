use command_macros::pagination;
use eyre::{Result, WrapErr};
use hashbrown::HashMap;
use rosu_v2::prelude::{Score, User};
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::TopScoreOrder,
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    database::MinimizedPp,
    embeds::{CondensedTopEmbed, EmbedData, TopEmbed, TopSingleEmbed},
    util::hasher::SimpleBuildHasher,
};

use super::Pages;

#[pagination(per_page = 5, entries = "scores")]
pub struct TopPagination {
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopScoreOrder,
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool), SimpleBuildHasher>,
}

impl TopPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        let embed_fut = TopEmbed::new(&self.user, scores, ctx, self.sort_by, &self.farm, pages);

        embed_fut.await.build()
    }
}

#[pagination(per_page = 10, entries = "scores")]
pub struct TopCondensedPagination {
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopScoreOrder,
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool), SimpleBuildHasher>,
}

impl TopCondensedPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        let embed_fut =
            CondensedTopEmbed::new(&self.user, scores, ctx, self.sort_by, &self.farm, pages);

        embed_fut.await.build()
    }
}

#[pagination(per_page = 1, entries = "scores")]
pub struct TopSinglePagination {
    user: User,
    scores: Vec<(usize, Score)>,
    minimized_pp: MinimizedPp,
}

impl TopSinglePagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let (idx, score) = self.scores.get(pages.index).unwrap();

        TopSingleEmbed::new(&self.user, score, Some(*idx), None, self.minimized_pp, ctx)
            .await
            .map(TopSingleEmbed::into_minimized)
            .wrap_err("failed to create embed data")
    }
}

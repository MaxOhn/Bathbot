use command_macros::pagination;
use hashbrown::HashMap;
use rosu_v2::prelude::{Score, User};
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::TopScoreOrder,
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    embeds::{CondensedTopEmbed, EmbedData, TopEmbed},
};

use super::Pages;

#[pagination(per_page = 5, entries = "scores")]
pub struct TopPagination {
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopScoreOrder,
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
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
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
}

impl TopCondensedPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        let embed_fut =
            CondensedTopEmbed::new(&self.user, scores, ctx, self.sort_by, &self.farm, pages);

        embed_fut.await.build()
    }
}

use command_macros::pagination;
use rosu_v2::prelude::{Score, User};
use twilight_model::channel::embed::Embed;

use crate::{
    core::Context,
    embeds::{EmbedData, NoChokeEmbed},
};

use super::Pages;

#[pagination(per_page = 5, entries = "scores")]
pub struct NoChokePagination {
    user: User,
    scores: Vec<(usize, Score, Score)>,
    unchoked_pp: f32,
    rank: Option<usize>,
}

impl NoChokePagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        let embed_fut =
            NoChokeEmbed::new(&self.user, scores, self.unchoked_pp, self.rank, ctx, pages);

        embed_fut.await.build()
    }
}

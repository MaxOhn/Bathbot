use command_macros::pagination;
use rosu_v2::prelude::{GameMode, Score, User};
use twilight_model::channel::embed::Embed;

use crate::embeds::{EmbedData, TopIfEmbed};

use super::Pages;

#[pagination(per_page = 5, entries = "scores")]
pub struct TopIfPagination {
    user: User,
    scores: Vec<(usize, Score, Option<f32>)>,
    mode: GameMode,
    pre_pp: f32,
    post_pp: f32,
    rank: Option<usize>,
}

impl TopIfPagination {
    pub async fn build_page(&mut self, pages: &Pages) -> Embed {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        let embed_fut = TopIfEmbed::new(
            &self.user,
            scores,
            self.mode,
            self.pre_pp,
            self.post_pp,
            self.rank,
            pages,
        );

        embed_fut.await.build()
    }
}

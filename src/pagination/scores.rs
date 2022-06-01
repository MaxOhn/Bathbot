use command_macros::pagination;
use rosu_v2::prelude::{Beatmap, Score, User};
use twilight_model::channel::embed::Embed;

use crate::{
    core::Context,
    embeds::{EmbedData, ScoresEmbed},
};

use super::Pages;

#[pagination(per_page = 10, entries = "scores")]
pub struct ScoresPagination {
    user: User,
    map: Beatmap,
    scores: Vec<Score>,
    pinned: Vec<Score>,
    personal: Vec<Score>,
    global_idx: Option<(usize, usize)>,
    pp_idx: usize,
}

impl ScoresPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        let global_idx = self
            .global_idx
            .filter(|(idx, _)| (pages.index..pages.index + pages.per_page).contains(idx))
            .map(|(score_idx, map_idx)| {
                let factor = score_idx / pages.per_page;
                let new_idx = score_idx - factor * pages.per_page;

                (new_idx, map_idx)
            });

        let embed_fut = ScoresEmbed::new(
            &self.user,
            &self.map,
            scores,
            &self.pinned,
            &self.personal,
            global_idx,
            self.pp_idx,
            pages,
            ctx,
        );

        embed_fut.await.build()
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use command_macros::BasePagination;
use rosu_v2::prelude::{Beatmap, Score, User};
use twilight_model::channel::Message;

use crate::{core::Context, embeds::ScoresEmbed, BotResult};

use super::{Pages, Pagination};

#[derive(BasePagination)]
#[pagination(single_step = 10)]
pub struct ScoresPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    map: Beatmap,
    scores: Vec<Score>,
    pinned: Vec<Score>,
    personal: Vec<Score>,
    global_idx: Option<(usize, usize)>,
    pp_idx: usize,
}

impl ScoresPagination {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        msg: Message,
        user: User,
        map: Beatmap,
        scores: Vec<Score>,
        pinned: Vec<Score>,
        personal: Vec<Score>,
        global_idx: Option<(usize, usize)>,
        pp_idx: usize,
        ctx: Arc<Context>,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(10, scores.len()),
            user,
            map,
            scores,
            pinned,
            personal,
            global_idx,
            pp_idx,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for ScoresPagination {
    type PageData = ScoresEmbed;

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let scores = self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page);

        let global_idx = self
            .global_idx
            .filter(|(idx, _)| {
                (self.pages.index..self.pages.index + self.pages.per_page).contains(idx)
            })
            .map(|(score_idx, map_idx)| {
                let factor = score_idx / self.pages.per_page;
                let new_idx = score_idx - factor * self.pages.per_page;

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
            (self.page(), self.pages.total_pages),
            &self.ctx,
        );

        Ok(embed_fut.await)
    }
}

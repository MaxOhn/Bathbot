use std::sync::Arc;

use async_trait::async_trait;
use rosu::model::Score as ScoreV1;
use rosu_v2::prelude::{Beatmap, Score, User};
use twilight_model::channel::Message;

use crate::{core::Context, embeds::ScoresEmbed, BotResult};

use super::{Pages, Pagination};

pub struct ScoresPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    map: Beatmap,
    scores: Vec<ScoreV1>,
    pinned: Vec<Score>,
    personal: Vec<Score>,
    global_idx: Option<(usize, usize)>,
}

impl ScoresPagination {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        msg: Message,
        user: User,
        map: Beatmap,
        scores: Vec<ScoreV1>,
        pinned: Vec<Score>,
        personal: Vec<Score>,
        global_idx: Option<(usize, usize)>,
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
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for ScoresPagination {
    type PageData = ScoresEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

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
            self.pages.index,
            &self.pinned,
            &self.personal,
            global_idx,
            &self.ctx,
        );

        Ok(embed_fut.await)
    }
}

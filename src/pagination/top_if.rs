use super::{Pages, Pagination};

use crate::{embeds::TopIfEmbed, BotResult};

use async_trait::async_trait;
use rosu::model::{Beatmap, GameMode, Score, User};
use twilight_model::channel::Message;

pub struct TopIfPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score, Beatmap, Option<f32>)>,
    mode: GameMode,
    pre_pp: f32,
    post_pp: f32,
}

impl TopIfPagination {
    pub fn new(
        msg: Message,
        user: User,
        scores: Vec<(usize, Score, Beatmap, Option<f32>)>,
        mode: GameMode,
        pre_pp: f32,
        post_pp: f32,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
            mode,
            pre_pp,
            post_pp,
        }
    }
}

#[async_trait]
impl Pagination for TopIfPagination {
    type PageData = TopIfEmbed;

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
        Ok(TopIfEmbed::new(
            &self.user,
            self.scores
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            self.mode,
            self.pre_pp,
            self.post_pp,
            (self.page(), self.pages.total_pages),
        )
        .await)
    }
}

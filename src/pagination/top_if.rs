use super::{Pages, Pagination};

use crate::{embeds::TopIfEmbed, BotResult};

use command_macros::BasePagination;
use rosu_v2::prelude::{GameMode, Score, User};
use twilight_model::channel::Message;

#[derive(BasePagination)]
#[pagination(single_step = 5)]
pub struct TopIfPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score, Option<f32>)>,
    mode: GameMode,
    pre_pp: f32,
    post_pp: f32,
    rank: Option<usize>,
}

impl TopIfPagination {
    pub fn new(
        msg: Message,
        user: User,
        scores: Vec<(usize, Score, Option<f32>)>,
        mode: GameMode,
        pre_pp: f32,
        post_pp: f32,
        rank: Option<usize>,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
            mode,
            pre_pp,
            post_pp,
            rank,
        }
    }
}

#[async_trait]
impl Pagination for TopIfPagination {
    type PageData = TopIfEmbed;

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let embed_fut = TopIfEmbed::new(
            &self.user,
            self.scores
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            self.mode,
            self.pre_pp,
            self.post_pp,
            self.rank,
            (self.page(), self.pages.total_pages),
        );

        Ok(embed_fut.await)
    }
}

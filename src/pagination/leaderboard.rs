use std::sync::Arc;

use command_macros::BasePagination;
use rosu_v2::{model::beatmap::Beatmap, prelude::Username};
use twilight_model::channel::Message;

use crate::{core::Context, custom_client::ScraperScore, embeds::LeaderboardEmbed, BotResult};

use super::{Pages, Pagination};

#[derive(BasePagination)]
#[pagination(single_step = 10)]
pub struct LeaderboardPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    map: Beatmap,
    scores: Vec<ScraperScore>,
    author_name: Option<Username>,
    first_place_icon: Option<String>,
}

impl LeaderboardPagination {
    pub fn new(
        msg: Message,
        map: Beatmap,
        scores: Vec<ScraperScore>,
        author_name: Option<Username>,
        first_place_icon: Option<String>,
        ctx: Arc<Context>,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(10, scores.len()),
            map,
            scores,
            author_name,
            first_place_icon,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for LeaderboardPagination {
    type PageData = LeaderboardEmbed;

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let scores = self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page);

        let embed_fut = LeaderboardEmbed::new(
            self.author_name.as_deref(),
            &self.map,
            Some(scores),
            &self.first_place_icon,
            self.pages.index,
            &self.ctx,
            (self.page(), self.pages.total_pages),
        );

        embed_fut.await
    }
}

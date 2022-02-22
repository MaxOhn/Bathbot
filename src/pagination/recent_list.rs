use std::sync::Arc;

use rosu_v2::prelude::{Score, User};
use twilight_model::channel::Message;

use crate::{embeds::RecentListEmbed, BotResult, Context};

use super::{Pages, Pagination};

pub struct RecentListPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<Score>,
}

impl RecentListPagination {
    pub fn new(msg: Message, user: User, scores: Vec<Score>, ctx: Arc<Context>) -> Self {
        Self {
            msg,
            user,
            pages: Pages::new(10, scores.len()),
            scores,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for RecentListPagination {
    type PageData = RecentListEmbed;

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

    async fn final_processing(mut self, ctx: &Context) -> BotResult<()> {
        // Set maps on garbage collection list if unranked
        for map in self.scores.iter().filter_map(|s| s.map.as_ref()) {
            ctx.map_garbage_collector(map).execute(ctx).await;
        }

        Ok(())
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let scores = self.scores.iter().skip(self.pages.index).take(10);

        RecentListEmbed::new(
            &self.user,
            scores,
            &self.ctx,
            (self.page(), self.pages.total_pages),
        )
        .await
    }
}

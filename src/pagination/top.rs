use std::sync::Arc;

use hashbrown::HashMap;
use rosu_v2::prelude::{Score, User};
use twilight_model::channel::Message;

use crate::{
    commands::osu::TopScoreOrder,
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    embeds::{CondensedTopEmbed, TopEmbed},
    BotResult,
};

use super::{Pages, Pagination};

pub struct TopPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopScoreOrder,
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
}

impl TopPagination {
    pub fn new(
        msg: Message,
        user: User,
        scores: Vec<(usize, Score)>,
        sort_by: TopScoreOrder,
        farm: HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
        ctx: Arc<Context>,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            scores,
            ctx,
            sort_by,
            farm,
        }
    }
}

pub struct CondensedTopPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopScoreOrder,
    farm: HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
}

impl CondensedTopPagination {
    pub fn new(
        msg: Message,
        user: User,
        scores: Vec<(usize, Score)>,
        sort_by: TopScoreOrder,
        farm: HashMap<u32, (OsuTrackerMapsetEntry, bool)>,
        ctx: Arc<Context>,
    ) -> Self {
        Self {
            pages: Pages::new(10, scores.len()),
            msg,
            user,
            scores,
            sort_by,
            farm,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for TopPagination {
    type PageData = TopEmbed;

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

        let pages = (self.page(), self.pages.total_pages);

        let embed_fut = TopEmbed::new(
            &self.user,
            scores,
            &self.ctx,
            self.sort_by,
            &self.farm,
            pages,
        );

        Ok(embed_fut.await)
    }
}

#[async_trait]
impl Pagination for CondensedTopPagination {
    type PageData = CondensedTopEmbed;

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

        let pages = (self.page(), self.pages.total_pages);

        let embed_fut = CondensedTopEmbed::new(
            &self.user,
            scores,
            &self.ctx,
            self.sort_by,
            &self.farm,
            pages,
        );

        Ok(embed_fut.await)
    }
}

use super::{Pages, Pagination};
use crate::{
    commands::osu::Difference, custom_client::SnipeRecent, embeds::SnipedDiffEmbed, BotResult,
};

use async_trait::async_trait;
use rosu::model::User;
use rosu_pp::Beatmap;
use std::collections::HashMap;
use twilight_model::channel::Message;

pub struct SnipedDiffPagination {
    msg: Message,
    pages: Pages,
    user: User,
    diff: Difference,
    scores: Vec<SnipeRecent>,
    maps: HashMap<u32, Beatmap>,
}

impl SnipedDiffPagination {
    pub fn new(
        msg: Message,
        user: User,
        diff: Difference,
        scores: Vec<SnipeRecent>,
        maps: HashMap<u32, Beatmap>,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            diff,
            scores,
            maps,
        }
    }
}

#[async_trait]
impl Pagination for SnipedDiffPagination {
    type PageData = SnipedDiffEmbed;

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
        SnipedDiffEmbed::new(
            &self.user,
            self.diff,
            &self.scores,
            self.pages.index,
            (self.page(), self.pages.total_pages),
            &mut self.maps,
        )
        .await
    }
}

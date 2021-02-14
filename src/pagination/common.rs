use super::{Pages, Pagination};

use crate::{
    commands::osu::CommonUser,
    embeds::{CommonEmbed, MapScores},
    BotResult,
};

use async_trait::async_trait;
use twilight_model::channel::Message;

pub struct CommonPagination {
    msg: Message,
    pages: Pages,
    users: Vec<CommonUser>,
    map_scores: MapScores,
    id_pps: Vec<(u32, f32)>,
}

impl CommonPagination {
    pub fn new(
        msg: Message,
        users: Vec<CommonUser>,
        map_scores: MapScores,
        id_pps: Vec<(u32, f32)>,
    ) -> Self {
        Self {
            pages: Pages::new(10, map_scores.len()),
            msg,
            users,
            map_scores,
            id_pps,
        }
    }
}

#[async_trait]
impl Pagination for CommonPagination {
    type PageData = CommonEmbed;

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
        Ok(CommonEmbed::new(
            &self.users,
            &self.map_scores,
            &self.id_pps[self.pages.index..(self.pages.index + 10).min(self.id_pps.len())],
            self.pages.index,
        ))
    }
}

use super::{Pages, Pagination};

use crate::{commands::osu::CommonUser, embeds::CommonEmbed, BotResult};

use async_trait::async_trait;
use rosu_v2::model::score::Score;
use twilight_model::channel::Message;

pub struct CommonPagination {
    msg: Message,
    pages: Pages,
    users: Vec<CommonUser>,
    scores_per_map: Vec<Vec<(usize, f32, Score)>>,
}

impl CommonPagination {
    pub fn new(
        msg: Message,
        users: Vec<CommonUser>,
        scores_per_map: Vec<Vec<(usize, f32, Score)>>,
    ) -> Self {
        Self {
            pages: Pages::new(10, scores_per_map.len()),
            msg,
            users,
            scores_per_map,
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
            &self.scores_per_map
                [self.pages.index..(self.pages.index + 10).min(self.scores_per_map.len())],
            self.pages.index,
        ))
    }
}

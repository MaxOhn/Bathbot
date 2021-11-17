use super::{Pages, Pagination};

use crate::{
    commands::osu::{CommonScoreEntry, CommonUser},
    embeds::CommonEmbed,
    BotResult,
};

use smallvec::SmallVec;
use twilight_model::channel::Message;

pub struct CommonPagination {
    msg: Message,
    pages: Pages,
    users: SmallVec<[CommonUser; 3]>,
    scores_per_map: Vec<SmallVec<[CommonScoreEntry; 3]>>,
}

impl CommonPagination {
    pub fn new(
        msg: Message,
        users: SmallVec<[CommonUser; 3]>,
        scores_per_map: Vec<SmallVec<[CommonScoreEntry; 3]>>,
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

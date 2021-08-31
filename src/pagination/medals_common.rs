use super::{Pages, Pagination};

use crate::{
    database::OsuMedal,
    embeds::{MedalsCommonEmbed, MedalsCommonUser},
    BotResult,
};

use async_trait::async_trait;
use twilight_model::channel::Message;

pub struct MedalsCommonPagination {
    msg: Message,
    pages: Pages,
    user1: MedalsCommonUser,
    user2: MedalsCommonUser,
    medals: Vec<OsuMedal>,
}

impl MedalsCommonPagination {
    pub fn new(
        msg: Message,
        user1: MedalsCommonUser,
        user2: MedalsCommonUser,
        medals: Vec<OsuMedal>,
    ) -> Self {
        Self {
            pages: Pages::new(10, medals.len()),
            msg,
            user1,
            user2,
            medals,
        }
    }
}

#[async_trait]
impl Pagination for MedalsCommonPagination {
    type PageData = MedalsCommonEmbed;

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
        let index = self.pages.index;
        let medals = &self.medals[index..(index + 10).min(self.medals.len())];
        let embed = MedalsCommonEmbed::new(&self.user1, &self.user2, medals, index);

        Ok(embed)
    }
}

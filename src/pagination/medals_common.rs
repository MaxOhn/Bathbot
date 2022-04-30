use super::{Pages, Pagination};

use crate::{
    commands::osu::MedalEntryCommon,
    embeds::{MedalsCommonEmbed, MedalsCommonUser},
    BotResult,
};

use command_macros::BasePagination;
use twilight_model::channel::Message;

#[derive(BasePagination)]
#[pagination(no_multi)]
pub struct MedalsCommonPagination {
    msg: Message,
    pages: Pages,
    user1: MedalsCommonUser,
    user2: MedalsCommonUser,
    medals: Vec<MedalEntryCommon>,
}

impl MedalsCommonPagination {
    pub fn new(
        msg: Message,
        user1: MedalsCommonUser,
        user2: MedalsCommonUser,
        medals: Vec<MedalEntryCommon>,
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

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let index = self.pages.index;
        let medals = &self.medals[index..(index + 10).min(self.medals.len())];
        let embed = MedalsCommonEmbed::new(&self.user1, &self.user2, medals, index);

        Ok(embed)
    }
}

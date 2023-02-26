use bathbot_macros::pagination;
use twilight_model::channel::message::embed::Embed;

use crate::{
    commands::osu::MedalEntryCommon,
    embeds::{EmbedData, MedalsCommonEmbed, MedalsCommonUser},
};

use super::Pages;

#[pagination(per_page = 10, entries = "medals")]
pub struct MedalsCommonPagination {
    user1: MedalsCommonUser,
    user2: MedalsCommonUser,
    medals: Vec<MedalEntryCommon>,
}

impl MedalsCommonPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index();
        let medals = &self.medals[idx..self.medals.len().min(idx + pages.per_page())];

        MedalsCommonEmbed::new(&self.user1, &self.user2, medals, pages).build()
    }
}

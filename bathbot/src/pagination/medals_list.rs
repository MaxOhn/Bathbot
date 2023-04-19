use bathbot_macros::pagination;
use bathbot_model::rosu_v2::user::User;
use twilight_model::channel::message::embed::Embed;

use super::Pages;
use crate::{
    commands::osu::MedalEntryList,
    embeds::{EmbedData, MedalsListEmbed},
    manager::redis::RedisData,
};

#[pagination(per_page = 10, entries = "medals")]
pub struct MedalsListPagination {
    user: RedisData<User>,
    acquired: (usize, usize),
    medals: Vec<MedalEntryList>,
}

impl MedalsListPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index();
        let limit = self.medals.len().min(idx + pages.per_page());

        MedalsListEmbed::new(&self.user, &self.medals[idx..limit], self.acquired, pages).build()
    }
}

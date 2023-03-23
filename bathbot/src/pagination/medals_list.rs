use bathbot_macros::pagination;
use twilight_model::channel::message::embed::Embed;

use crate::{
    commands::osu::MedalEntryList,
    embeds::{EmbedData, MedalsListEmbed},
    manager::redis::{osu::User, RedisData},
};

use super::Pages;

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

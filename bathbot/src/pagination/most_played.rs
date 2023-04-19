use bathbot_macros::pagination;
use bathbot_model::rosu_v2::user::User;
use rosu_v2::prelude::MostPlayedMap;
use twilight_model::channel::message::embed::Embed;

use super::Pages;
use crate::{
    embeds::{EmbedData, MostPlayedEmbed},
    manager::redis::RedisData,
};

#[pagination(per_page = 10, entries = "maps")]
pub struct MostPlayedPagination {
    user: RedisData<User>,
    maps: Vec<MostPlayedMap>,
}

impl MostPlayedPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.maps.len().min(pages.index() + pages.per_page());
        let maps = &self.maps[pages.index()..end_idx];

        MostPlayedEmbed::new(&self.user, maps, pages).build()
    }
}

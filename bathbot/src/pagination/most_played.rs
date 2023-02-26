use bathbot_macros::pagination;
use rosu_v2::prelude::MostPlayedMap;
use twilight_model::channel::message::embed::Embed;

use crate::{
    embeds::{EmbedData, MostPlayedEmbed},
    manager::redis::{osu::User, RedisData},
};

use super::Pages;

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

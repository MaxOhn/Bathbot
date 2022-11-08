use command_macros::pagination;
use hashbrown::HashMap;
use rosu_v2::prelude::MostPlayedMap;
use twilight_model::channel::embed::Embed;

use crate::{
    embeds::{EmbedData, MostPlayedCommonEmbed},
    manager::redis::{osu::User, RedisData},
};

use super::Pages;

#[pagination(per_page = 10, entries = "maps")]
pub struct MostPlayedCommonPagination {
    user1: RedisData<User>,
    user2: RedisData<User>,
    maps: HashMap<u32, ([usize; 2], MostPlayedMap)>,
    map_counts: Vec<(u32, usize)>,
}

impl MostPlayedCommonPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index;
        let map_counts = &self.map_counts[idx..self.maps.len().min(idx + pages.per_page)];

        MostPlayedCommonEmbed::new(&self.user1, &self.user2, map_counts, &self.maps, pages).build()
    }
}

use command_macros::pagination;
use hashbrown::HashMap;
use rosu_v2::prelude::{MostPlayedMap, Username};
use twilight_model::channel::embed::Embed;

use crate::embeds::{EmbedData, MostPlayedCommonEmbed};

use super::Pages;

#[pagination(per_page = 10, entries = "maps")]
pub struct MostPlayedCommonPagination {
    name1: Username,
    name2: Username,
    maps: HashMap<u32, ([usize; 2], MostPlayedMap)>,
    map_counts: Vec<(u32, usize)>,
}

impl MostPlayedCommonPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index;
        let map_counts = &self.map_counts[idx..self.maps.len().min(idx + pages.per_page)];

        MostPlayedCommonEmbed::new(&self.name1, &self.name2, map_counts, &self.maps, &pages).build()
    }
}

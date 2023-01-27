use std::collections::HashMap;

use bathbot_macros::pagination;
use bathbot_util::IntHasher;
use rosu_v2::prelude::{Beatmap, BeatmapsetCompact, Username};
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::CommonScore,
    embeds::{CommonEmbed, EmbedData},
};

use super::Pages;

#[pagination(per_page = 10, entries = "maps")]
pub struct CommonPagination {
    name1: Username,
    name2: Username,
    maps: HashMap<u32, ([CommonScore; 2], Beatmap, BeatmapsetCompact), IntHasher>,
    map_pps: Vec<(u32, f32)>,
    wins: [u8; 2],
}

impl CommonPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index();
        let map_pps = &self.map_pps[idx..(idx + pages.per_page()).min(self.maps.len())];

        CommonEmbed::new(
            &self.name1,
            &self.name2,
            map_pps,
            &self.maps,
            self.wins,
            pages,
        )
        .build()
    }
}

use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, BeatmapsetCompact, Username};
use twilight_model::channel::Message;

use crate::{commands::osu::CommonScore, embeds::CommonEmbed, BotResult};

use super::{Pages, Pagination};

pub struct CommonPagination {
    msg: Message,
    pages: Pages,
    name1: Username,
    name2: Username,
    maps: HashMap<u32, ([CommonScore; 2], Beatmap, BeatmapsetCompact)>,
    map_pps: Vec<(u32, f32)>,
    wins: [u8; 2],
}

impl CommonPagination {
    pub fn new(
        msg: Message,
        name1: Username,
        name2: Username,
        maps: HashMap<u32, ([CommonScore; 2], Beatmap, BeatmapsetCompact)>,
        map_pps: Vec<(u32, f32)>,
        wins: [u8; 2],
    ) -> Self {
        Self {
            pages: Pages::new(10, maps.len()),
            msg,
            name1,
            name2,
            maps,
            map_pps,
            wins,
        }
    }
}

#[async_trait]
impl Pagination for CommonPagination {
    type PageData = CommonEmbed;

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
        Ok(CommonEmbed::new(
            &self.name1,
            &self.name2,
            &self.map_pps[self.pages.index..(self.pages.index + 10).min(self.maps.len())],
            &self.maps,
            self.wins,
            self.pages.index,
        ))
    }
}

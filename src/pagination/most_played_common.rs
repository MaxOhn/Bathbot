use hashbrown::HashMap;
use rosu_v2::prelude::{MostPlayedMap, Username};
use twilight_model::channel::Message;

use crate::{embeds::MostPlayedCommonEmbed, BotResult};

use super::{Pages, Pagination};

pub struct MostPlayedCommonPagination {
    msg: Message,
    pages: Pages,
    name1: Username,
    name2: Username,
    maps: HashMap<u32, ([usize; 2], MostPlayedMap)>,
    map_counts: Vec<(u32, usize)>,
}

impl MostPlayedCommonPagination {
    pub fn new(
        msg: Message,
        name1: Username,
        name2: Username,
        maps: HashMap<u32, ([usize; 2], MostPlayedMap)>,
        map_counts: Vec<(u32, usize)>,
    ) -> Self {
        Self {
            pages: Pages::new(10, maps.len()),
            msg,
            name1,
            name2,
            maps,
            map_counts,
        }
    }
}

#[async_trait]
impl Pagination for MostPlayedCommonPagination {
    type PageData = MostPlayedCommonEmbed;

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
        Ok(MostPlayedCommonEmbed::new(
            &self.name1,
            &self.name2,
            &self.map_counts[self.pages.index..(self.pages.index + 10).min(self.maps.len())],
            &self.maps,
            self.pages.index,
        ))
    }
}

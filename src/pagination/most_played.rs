use command_macros::pagination;
use rosu_v2::prelude::{MostPlayedMap, User};
use twilight_model::channel::embed::Embed;

use crate::embeds::{EmbedData, MostPlayedEmbed};

use super::Pages;

#[pagination(per_page = 10, entries = "maps")]
pub struct MostPlayedPagination {
    user: User,
    maps: Vec<MostPlayedMap>,
}

impl MostPlayedPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let maps = self.maps.iter().skip(pages.index).take(pages.per_page);

        MostPlayedEmbed::new(&self.user, maps, pages).build()
    }
}

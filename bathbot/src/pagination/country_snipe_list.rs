use bathbot_macros::pagination;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::SnipeCountryListOrder,
    custom_client::SnipeCountryPlayer,
    embeds::{CountrySnipeListEmbed, EmbedData},
    util::CountryCode,
};

use super::Pages;

#[pagination(per_page = 10, entries = "players")]
pub struct CountrySnipeListPagination {
    players: Vec<(usize, SnipeCountryPlayer)>,
    country: Option<(String, CountryCode)>,
    order: SnipeCountryListOrder,
    author_idx: Option<usize>,
}

impl CountrySnipeListPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let players = self.players.iter().skip(pages.index).take(pages.per_page);
        let country = self.country.as_ref();

        CountrySnipeListEmbed::new(country, self.order, players, self.author_idx, pages).build()
    }
}

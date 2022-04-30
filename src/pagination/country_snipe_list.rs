use command_macros::BasePagination;
use twilight_model::channel::Message;

use crate::{
    commands::osu::SnipeCountryListOrder, custom_client::SnipeCountryPlayer,
    embeds::CountrySnipeListEmbed, util::CountryCode, BotResult,
};

use super::{Pages, Pagination};

#[derive(BasePagination)]
#[jump_idx(author_idx)]
pub struct CountrySnipeListPagination {
    msg: Message,
    pages: Pages,
    players: Vec<(usize, SnipeCountryPlayer)>,
    country: Option<(String, CountryCode)>,
    order: SnipeCountryListOrder,
    author_idx: Option<usize>,
}

impl CountrySnipeListPagination {
    pub fn new(
        msg: Message,
        players: Vec<(usize, SnipeCountryPlayer)>,
        country: Option<(String, CountryCode)>,
        order: SnipeCountryListOrder,
        author_idx: Option<usize>,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(10, players.len()),
            players,
            country,
            order,
            author_idx,
        }
    }
}

#[async_trait]
impl Pagination for CountrySnipeListPagination {
    type PageData = CountrySnipeListEmbed;

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        Ok(CountrySnipeListEmbed::new(
            self.country.as_ref(),
            self.order,
            self.players
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            self.author_idx,
            (self.page(), self.pages.total_pages),
        ))
    }
}

use super::{Pages, Pagination, ReactionVec};

use crate::{
    commands::osu::SnipeOrder, custom_client::SnipeCountryPlayer, embeds::CountrySnipeListEmbed,
    util::Emote, BotResult, CountryCode,
};

use async_trait::async_trait;
use twilight_model::channel::Message;

pub struct CountrySnipeListPagination {
    msg: Message,
    pages: Pages,
    players: Vec<(usize, SnipeCountryPlayer)>,
    country: Option<(String, CountryCode)>,
    order: SnipeOrder,
    author_idx: Option<usize>,
}

impl CountrySnipeListPagination {
    pub fn new(
        msg: Message,
        players: Vec<(usize, SnipeCountryPlayer)>,
        country: Option<(String, CountryCode)>,
        order: SnipeOrder,
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

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn jump_index(&self) -> Option<usize> {
        self.author_idx
    }

    fn reactions() -> ReactionVec {
        smallvec![
            Emote::JumpStart,
            Emote::MultiStepBack,
            Emote::SingleStepBack,
            Emote::MyPosition,
            Emote::SingleStep,
            Emote::MultiStep,
            Emote::JumpEnd,
        ]
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    fn multi_step(&self) -> usize {
        self.pages.per_page * 5
    }

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

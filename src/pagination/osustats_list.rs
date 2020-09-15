use super::{Pages, Pagination};
use crate::{
    custom_client::{OsuStatsListParams, OsuStatsPlayer},
    embeds::OsuStatsListEmbed,
    BotResult, Context,
};

use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::channel::Message;

pub struct OsuStatsListPagination {
    msg: Message,
    pages: Pages,
    players: HashMap<usize, Vec<OsuStatsPlayer>>,
    params: OsuStatsListParams,
    first_place_id: u32,
    ctx: Arc<Context>,
}

impl OsuStatsListPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        players: HashMap<usize, Vec<OsuStatsPlayer>>,
        params: OsuStatsListParams,
        amount: usize,
    ) -> Self {
        let first_place_id = players[&1].first().unwrap().user_id;
        Self {
            pages: Pages::new(15, amount),
            msg,
            players,
            params,
            first_place_id,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for OsuStatsListPagination {
    type PageData = OsuStatsListEmbed;
    fn msg(&self) -> &Message {
        &self.msg
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    fn reactions() -> Vec<RequestReactionType> {
        Self::arrow_reactions_full()
    }
    fn single_step(&self) -> usize {
        15
    }
    fn multi_step(&self) -> usize {
        45
    }
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let page = self.page();
        #[allow(clippy::map_entry)]
        if !self.players.contains_key(&page) {
            self.params.page = page;
            let players = self
                .ctx
                .clients
                .custom
                .get_country_globals(&self.params)
                .await?;
            self.players.insert(page, players);
        }
        let players = self.players.get(&page).unwrap();
        Ok(OsuStatsListEmbed::new(
            players,
            &self.params.country,
            self.first_place_id,
            (self.page(), self.pages.total_pages),
        ))
    }
}

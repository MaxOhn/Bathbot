use super::{Pages, Pagination, ReactionVec};
use crate::{
    commands::osu::OsuStatsPlayersArgs, custom_client::OsuStatsPlayer, embeds::OsuStatsListEmbed,
    BotResult, Context,
};

use command_macros::BasePagination;
use hashbrown::HashMap;
use std::sync::Arc;
use twilight_model::channel::Message;

#[derive(BasePagination)]
#[pagination(single_step = 15, multi_step = 45)]
pub struct OsuStatsListPagination {
    msg: Message,
    pages: Pages,
    players: HashMap<usize, Vec<OsuStatsPlayer>>,
    params: OsuStatsPlayersArgs,
    first_place_id: u32,
    ctx: Arc<Context>,
}

impl OsuStatsListPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        players: HashMap<usize, Vec<OsuStatsPlayer>>,
        params: OsuStatsPlayersArgs,
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

    fn reactions() -> ReactionVec {
        Self::arrow_reactions_full()
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let page = self.page();

        #[allow(clippy::map_entry)]
        if !self.players.contains_key(&page) {
            self.params.page = page;

            let players = self.ctx.client().get_country_globals(&self.params).await?;

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

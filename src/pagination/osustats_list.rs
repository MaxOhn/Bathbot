use hashbrown::{hash_map::Entry, HashMap};
use std::sync::Arc;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::OsuStatsPlayersArgs,
    custom_client::OsuStatsPlayer,
    embeds::{EmbedData, OsuStatsListEmbed},
    BotResult, Context,
};

use super::{Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct OsuStatsListPagination {
    ctx: Arc<Context>,
    players: HashMap<usize, Vec<OsuStatsPlayer>>,
    params: OsuStatsPlayersArgs,
    first_place_id: u32,
}

impl OsuStatsListPagination {
    pub fn builder(
        ctx: Arc<Context>,
        players: HashMap<usize, Vec<OsuStatsPlayer>>,
        params: OsuStatsPlayersArgs,
        first_place_id: u32,
        amount: usize,
    ) -> PaginationBuilder {
        let pagination = Self {
            ctx,
            players,
            params,
            first_place_id,
        };

        let pages = Pages::new(15, amount);
        let kind = PaginationKind::OsuStatsList(pagination);

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, pages: &Pages) -> BotResult<Embed> {
        let page = pages.curr_page();

        let players = match self.players.entry(page) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                self.params.page = page;
                let players = self.ctx.client().get_country_globals(&self.params).await?;

                e.insert(players)
            }
        };

        let embed =
            OsuStatsListEmbed::new(players, &self.params.country, self.first_place_id, pages);

        Ok(embed.build())
    }
}

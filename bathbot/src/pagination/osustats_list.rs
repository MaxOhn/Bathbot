use bathbot_model::{OsuStatsPlayer, OsuStatsPlayersArgs};
use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use hashbrown::{hash_map::Entry, HashMap};
use twilight_model::channel::message::embed::Embed;

use crate::{
    embeds::{EmbedData, OsuStatsListEmbed},
    Context,
};

use super::{Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct OsuStatsListPagination {
    players: HashMap<usize, Vec<OsuStatsPlayer>, IntHasher>,
    params: OsuStatsPlayersArgs,
    first_place_id: u32,
}

impl OsuStatsListPagination {
    pub fn builder(
        players: HashMap<usize, Vec<OsuStatsPlayer>, IntHasher>,
        params: OsuStatsPlayersArgs,
        first_place_id: u32,
        amount: usize,
    ) -> PaginationBuilder {
        let pagination = Self {
            players,
            params,
            first_place_id,
        };

        let pages = Pages::new(15, amount);
        let kind = PaginationKind::OsuStatsList(Box::new(pagination));

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let page = pages.curr_page();

        let players = match self.players.entry(page) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                self.params.page = page;

                let players = ctx
                    .client()
                    .get_country_globals(&self.params)
                    .await
                    .wrap_err("failed to get country globals")?;

                e.insert(players)
            }
        };

        let embed =
            OsuStatsListEmbed::new(players, &self.params.country, self.first_place_id, pages);

        Ok(embed.build())
    }
}

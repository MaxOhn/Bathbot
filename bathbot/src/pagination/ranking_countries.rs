use bathbot_macros::pagination;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::{CountryRanking, GameMode};
use std::collections::BTreeMap;
use twilight_model::channel::embed::Embed;

use crate::{
    embeds::{EmbedData, RankingCountriesEmbed},
    Context,
};

use super::Pages;

#[pagination(per_page = 15, total = "total")]
pub struct RankingCountriesPagination {
    mode: GameMode,
    countries: BTreeMap<usize, CountryRanking>,
    total: usize,
}

impl RankingCountriesPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let count = self
            .countries
            .range(pages.index..pages.index + pages.per_page)
            .count();

        if count < pages.per_page && count < self.total - pages.index {
            // * If the amount of countries changes to 240-255,
            // * two request will need to be done to skip to the end
            let page = match pages.index {
                45 => 2,
                90 if !self.countries.contains_key(&90) => 2, // when going back to front
                90 | 135 => 3,
                150 => 4,
                195 if !self.countries.contains_key(&195) => 4, // when going back to front
                195 | 225 => 5,
                240 => 5, // technically 6 but there are currently <250 countries so there is no page 6
                _ => bail!("unexpected page index {}", pages.index),
            };

            let offset = page - 1;

            let mut ranking = ctx
                .osu()
                .country_rankings(self.mode)
                .page(page as u32)
                .await
                .wrap_err("failed to get country rankings")?;

            let iter = ranking
                .ranking
                .drain(..)
                .enumerate()
                .map(|(i, country)| (offset * 50 + i, country));

            self.countries.extend(iter);
        }

        let embed = RankingCountriesEmbed::new(self.mode, &self.countries, pages);

        Ok(embed.build())
    }
}

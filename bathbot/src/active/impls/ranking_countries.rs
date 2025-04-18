use std::{collections::BTreeMap, fmt::Write};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{EmbedBuilder, FooterBuilder, numbers::WithComma};
use eyre::{Result, WrapErr};
use rosu_v2::prelude::{CountryRanking, GameMode};
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    core::Context,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct RankingCountriesPagination {
    mode: GameMode,
    #[pagination(per_page = 15, len = "total")]
    countries: BTreeMap<usize, CountryRanking>,
    total: usize,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for RankingCountriesPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;

        let count = self
            .countries
            .range(pages.index()..pages.index() + pages.per_page())
            .count();

        if count < pages.per_page() && count < self.total - pages.index() {
            // * If the amount of countries changes to 240-255,
            // * two request will need to be done to skip to the end
            let page = match pages.index() {
                45 => 2,
                90 if !self.countries.contains_key(&90) => 2, // when going back to front
                90 | 135 => 3,
                150 => 4,
                195 if !self.countries.contains_key(&195) => 4, // when going back to front
                195 | 225 => 5,
                // technically 6 but there are currently <250 countries so there is no page 6
                240 => 5,
                _ => bail!("Unexpected page index {}", pages.index()),
            };

            let offset = page - 1;

            let mut ranking = Context::osu()
                .country_rankings(self.mode)
                .page(page as u32)
                .await
                .wrap_err("Failed to get country rankings")?;

            let iter = ranking
                .ranking
                .drain(..)
                .enumerate()
                .map(|(i, country)| (offset * 50 + i, country));

            self.countries.extend(iter);
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages}");

        let index = (page - 1) * 15;

        let mut idx_len = 0;
        let mut name_len = 0;
        let mut pp_len = 0;
        let mut users_len = 0;

        let mut buf = String::new();

        for (i, country) in self.countries.range(index..index + 15) {
            let mut idx = i + 1;
            let mut len = 0;

            while idx > 0 {
                len += 1;
                idx /= 10;
            }

            idx_len = idx_len.max(len);

            name_len = name_len.max(country.country.len());

            buf.clear();

            let _ = write!(buf, "{}", WithComma::new(country.pp as u64));
            pp_len = pp_len.max(buf.len());

            buf.clear();
            let _ = write!(buf, "{}", WithComma::new(country.active_users));
            users_len = users_len.max(buf.len());
        }

        let mut description = String::with_capacity(1100);

        for (i, country) in self.countries.range(index..index + 15) {
            let idx = i + 1;

            buf.clear();
            let _ = write!(buf, "{}", WithComma::new(country.pp as u64));

            let _ = write!(
                description,
                "`#{idx:<idx_len$}` :flag_{code}: `{name:<name_len$}` `{pp:>pp_len$}pp`",
                idx = idx,
                idx_len = idx_len,
                code = country.country_code.to_ascii_lowercase(),
                name = country.country,
                name_len = name_len,
                pp = buf,
                pp_len = pp_len,
            );

            buf.clear();
            let _ = write!(buf, "{}", WithComma::new(country.active_users));
            let _ = writeln!(description, " `{buf:>users_len$} users`");
        }

        let title = format!("Country Ranking for osu!{}", mode_str(self.mode));
        let url = format!("https://osu.ppy.sh/rankings/{}/country", self.mode);

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .title(title)
            .url(url);

        Ok(BuildPage::new(embed, true))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        handle_pagination_component(component, self.msg_owner, true, &mut self.pages).await
    }

    async fn handle_modal(&mut self, modal: &mut InteractionModal) -> Result<()> {
        handle_pagination_modal(modal, self.msg_owner, true, &mut self.pages).await
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "",
        GameMode::Taiko => "taiko",
        GameMode::Catch => "ctb",
        GameMode::Mania => "mania",
    }
}

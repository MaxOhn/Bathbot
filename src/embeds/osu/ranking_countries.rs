use crate::{embeds::Footer, util::numbers::with_comma_uint};

use rosu_v2::prelude::{CountryRanking, GameMode};
use std::{collections::BTreeMap, fmt::Write};

pub struct RankingCountriesEmbed {
    description: String,
    title: String,
    url: String,
    footer: Footer,
}

impl RankingCountriesEmbed {
    pub fn new(
        mode: GameMode,
        countries: &BTreeMap<usize, CountryRanking>,
        pages: (usize, usize),
    ) -> Self {
        let index = (pages.0 - 1) * 15;

        let mut idx_len = 0;
        let mut name_len = 0;
        let mut pp_len = 0;
        let mut users_len = 0;

        let mut buf = String::new();

        for (i, country) in countries.range(index..index + 15) {
            let mut idx = i + 1;
            let mut len = 0;

            while idx > 0 {
                len += 1;
                idx /= 10;
            }

            idx_len = idx_len.max(len);

            name_len = name_len.max(country.country.len());

            buf.clear();
            let _ = write!(buf, "{}", with_comma_uint(country.pp as u64));
            pp_len = pp_len.max(buf.len());

            buf.clear();
            let _ = write!(buf, "{}", with_comma_uint(country.active_users));
            users_len = users_len.max(buf.len());
        }

        let mut description = String::with_capacity(1100);

        for (i, country) in countries.range(index..index + 15) {
            let idx = i + 1;

            buf.clear();
            let _ = write!(buf, "{}", with_comma_uint(country.pp as u64));

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
            let _ = write!(buf, "{}", with_comma_uint(country.active_users));

            let _ = writeln!(
                description,
                " `{users:>users_len$} users`",
                users = buf,
                users_len = users_len
            );
        }

        Self {
            description,
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
            title: format!("Country Ranking for osu!{}", mode_str(mode)),
            url: format!("https://osu.ppy.sh/rankings/{}/country", mode),
        }
    }
}

impl_builder!(RankingCountriesEmbed {
    description,
    footer,
    title,
    url,
});

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::TKO => "taiko",
        GameMode::CTB => "ctb",
        GameMode::MNA => "mania",
    }
}

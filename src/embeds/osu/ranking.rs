use crate::{
    commands::osu::UserValue,
    embeds::{Author, Footer},
    util::{osu::flag_url, CountryCode},
    Name,
};

use rosu_v2::prelude::GameMode;
use std::{
    collections::{btree_map::Range, BTreeMap},
    fmt::Write,
};

pub struct RankingEntry {
    pub value: UserValue,
    pub name: Name,
    pub country: CountryCode,
}

pub enum RankingKindData {
    OsekaiRarity,
    OsekaiMedalCount,
    OsekaiReplays,
    OsekaiTotalPp,
    OsekaiStandardDeviation,
    OsekaiBadges,
    OsekaiRankedMapsets,
    OsekaiLovedMapsets,
    PpCountry {
        country: String,
        country_code: CountryCode,
        mode: GameMode,
    },
    PpGlobal {
        mode: GameMode,
    },
    RankedScore {
        mode: GameMode,
    },
}

impl RankingKindData {
    fn author(&self) -> Author {
        match self {
            RankingKindData::OsekaiRarity => {
                let text = "Medal Ranking based on rarity";
                let url = "https://osekai.net/rankings/?ranking=Medals&type=Rarity";

                Author::new(text).url(url)
            }
            RankingKindData::OsekaiMedalCount => {
                let text = "User Ranking based on amount of owned medals";
                let url = "https://osekai.net/rankings/?ranking=Medals&type=Users";

                Author::new(text).url(url)
            }
            RankingKindData::OsekaiReplays => {
                let text = "User Ranking based on watched replays";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Replays";

                Author::new(text).url(url)
            }
            RankingKindData::OsekaiTotalPp => {
                let text = "User Ranking based on total pp across all modes";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Total+pp";

                Author::new(text).url(url)
            }
            RankingKindData::OsekaiStandardDeviation => {
                let text = "User Ranking based on pp standard deviation of all modes";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Standard+Deviation";

                Author::new(text).url(url)
            }
            RankingKindData::OsekaiBadges => {
                let text = "User Ranking based on amount of badges";
                let url = "https://osekai.net/rankings/?ranking=Badges&type=Badges";

                Author::new(text).url(url)
            }
            RankingKindData::OsekaiRankedMapsets => {
                let text = "User Ranking based on created ranked mapsets";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Ranked+Mapsets";

                Author::new(text).url(url)
            }
            RankingKindData::OsekaiLovedMapsets => {
                let text = "User Ranking based on created loved mapsets";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Loved+Mapsets";

                Author::new(text).url(url)
            }
            Self::PpCountry {
                country,
                country_code,
                mode,
            } => {
                let text = format!(
                    "{name}'{plural} Performance Ranking for osu!{mode}",
                    name = country,
                    plural = if country.ends_with('s') { "" } else { "s" },
                    mode = mode_str(*mode),
                );

                let url = format!(
                    "https://osu.ppy.sh/rankings/{mode}/performance?country={country}",
                    mode = mode,
                    country = country_code
                );

                let icon_url = flag_url(country_code.as_str());

                Author::new(text).url(url).icon_url(icon_url)
            }
            Self::PpGlobal { mode } => {
                let text = format!("Performance Ranking for osu!{mode}", mode = mode_str(*mode));

                let url = format!(
                    "https://osu.ppy.sh/rankings/{mode}/performance",
                    mode = mode,
                );

                Author::new(text).url(url)
            }
            Self::RankedScore { mode } => {
                let text = format!(
                    "Ranked Score Ranking for osu!{mode}",
                    mode = mode_str(*mode),
                );

                let url = format!("https://osu.ppy.sh/rankings/{mode}/score", mode = mode);

                Author::new(text).url(url)
            }
        }
    }

    fn footer(&self, curr_page: usize, total_pages: usize, author_idx: Option<usize>) -> Footer {
        let mut text = format!("Page {}/{}", curr_page, total_pages);

        if let Some(idx) = author_idx {
            let _ = write!(text, " • Your position: {}", idx + 1);
        }

        match self {
            RankingKindData::OsekaiRarity
            | RankingKindData::OsekaiMedalCount
            | RankingKindData::OsekaiReplays
            | RankingKindData::OsekaiTotalPp
            | RankingKindData::OsekaiStandardDeviation
            | RankingKindData::OsekaiBadges
            | RankingKindData::OsekaiRankedMapsets
            | RankingKindData::OsekaiLovedMapsets => {
                text.push_str(" • Check out osekai.net for more info")
            }
            Self::PpCountry { .. } | Self::PpGlobal { .. } | Self::RankedScore { .. } => {}
        };

        Footer::new(text)
    }
}

pub struct RankingEmbed {
    description: String,
    author: Author,
    footer: Footer,
}

type RankingMap = BTreeMap<usize, RankingEntry>;

impl RankingEmbed {
    pub fn new(
        users: &RankingMap,
        data: &RankingKindData,
        author_idx: Option<usize>,
        pages: (usize, usize),
    ) -> Self {
        let index = (pages.0 - 1) * 20;

        let mut buf = String::new();

        let left_lengths = lengths(&mut buf, users.range(index..index + 10));
        let right_lengths = lengths(&mut buf, users.range(index + 10..index + 20));

        let mut description = String::with_capacity(1024);

        // Ensuring the right side has ten elements for the zip
        let user_iter = users
            .range(index..index + 10)
            .zip((10..20).map(|i| users.get(&(index + i))));

        for ((i, left_entry), right) in user_iter {
            let idx = i + 1;

            buf.clear();
            let _ = write!(buf, "{}", left_entry.value);

            let _ = write!(
                description,
                "`#{idx:<idx_len$}` :flag_{country}: `{name:<name_len$}` `{value:>value_len$}`",
                idx = idx,
                idx_len = left_lengths.idx,
                country = left_entry.country.to_ascii_lowercase(),
                name = left_entry.name,
                name_len = left_lengths.name,
                value = buf,
                value_len = left_lengths.value,
            );

            if let Some(right_entry) = right {
                buf.clear();
                let _ = write!(buf, "{}", right_entry.value);

                let _ = write!(
                    description,
                    " | `#{idx:<idx_len$}` :flag_{country}: `{name:<name_len$}` `{value:>value_len$}`",
                    idx = idx + 10,
                    idx_len = right_lengths.idx,
                    country = right_entry.country.to_ascii_lowercase(),
                    name = right_entry.name,
                    name_len = right_lengths.name,
                    value = buf,
                    value_len = right_lengths.value,
                );
            }

            description.push('\n');
        }

        Self {
            author: data.author(),
            description,
            footer: data.footer(pages.0, pages.1, author_idx),
        }
    }
}

impl_builder!(RankingEmbed {
    description,
    footer,
    author,
});

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::TKO => "taiko",
        GameMode::CTB => "ctb",
        GameMode::MNA => "mania",
    }
}

fn lengths(buf: &mut String, iter: Range<usize, RankingEntry>) -> Lengths {
    let mut idx_len = 0;
    let mut name_len = 0;
    let mut value_len = 0;

    for (i, entry) in iter {
        let mut idx = i + 1;
        let mut len = 0;

        while idx > 0 {
            len += 1;
            idx /= 10;
        }

        idx_len = idx_len.max(len);
        name_len = name_len.max(entry.name.len());

        buf.clear();
        let _ = write!(buf, "{}", entry.value);
        value_len = value_len.max(buf.len());
    }

    Lengths {
        idx: idx_len,
        name: name_len,
        value: value_len,
    }
}

struct Lengths {
    idx: usize,
    name: usize,
    value: usize,
}

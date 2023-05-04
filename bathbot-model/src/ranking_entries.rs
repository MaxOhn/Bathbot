use std::{collections::BTreeMap, fmt::Write, ops::RangeBounds};

use bathbot_util::{AuthorBuilder, FooterBuilder};
use rosu_v2::prelude::{CountryCode, GameMode, Username};
use time::OffsetDateTime;
use twilight_model::id::{marker::GuildMarker, Id};

use crate::{
    twilight_model::util::ImageHash, BgGameScore, HlGameScore, HlVersion, UserModeStatsColumn,
    UserStatsColumn, UserStatsEntries, UserStatsEntry,
};

pub struct RankingEntry<V> {
    pub country: Option<CountryCode>,
    pub name: Username,
    pub value: V,
}

impl<V> From<UserStatsEntry<V>> for RankingEntry<V> {
    #[inline]
    fn from(entry: UserStatsEntry<V>) -> Self {
        Self {
            country: Some(unsafe { CountryCode::from_buf_unchecked(entry.country) }),
            name: entry.name.into(),
            value: entry.value,
        }
    }
}

pub enum RankingEntries {
    Accuracy(BTreeMap<usize, RankingEntry<f32>>),
    Amount(BTreeMap<usize, RankingEntry<u64>>),
    AmountWithNegative(BTreeMap<usize, RankingEntry<i64>>),
    Date(BTreeMap<usize, RankingEntry<OffsetDateTime>>),
    Float(BTreeMap<usize, RankingEntry<f32>>),
    Playtime(BTreeMap<usize, RankingEntry<u32>>),
    PpF32(BTreeMap<usize, RankingEntry<f32>>),
    PpU32(BTreeMap<usize, RankingEntry<u32>>),
    Rank(BTreeMap<usize, RankingEntry<u32>>),
}

impl RankingEntries {
    pub fn contains_key(&self, key: usize) -> bool {
        match self {
            RankingEntries::Accuracy(entries) => entries.contains_key(&key),
            RankingEntries::Amount(entries) => entries.contains_key(&key),
            RankingEntries::AmountWithNegative(entries) => entries.contains_key(&key),
            RankingEntries::Date(entries) => entries.contains_key(&key),
            RankingEntries::Float(entries) => entries.contains_key(&key),
            RankingEntries::Playtime(entries) => entries.contains_key(&key),
            RankingEntries::PpF32(entries) => entries.contains_key(&key),
            RankingEntries::PpU32(entries) => entries.contains_key(&key),
            RankingEntries::Rank(entries) => entries.contains_key(&key),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            RankingEntries::Accuracy(entries) => entries.is_empty(),
            RankingEntries::Amount(entries) => entries.is_empty(),
            RankingEntries::AmountWithNegative(entries) => entries.is_empty(),
            RankingEntries::Date(entries) => entries.is_empty(),
            RankingEntries::Float(entries) => entries.is_empty(),
            RankingEntries::Playtime(entries) => entries.is_empty(),
            RankingEntries::PpF32(entries) => entries.is_empty(),
            RankingEntries::PpU32(entries) => entries.is_empty(),
            RankingEntries::Rank(entries) => entries.is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            RankingEntries::Accuracy(entries) => entries.len(),
            RankingEntries::Amount(entries) => entries.len(),
            RankingEntries::AmountWithNegative(entries) => entries.len(),
            RankingEntries::Date(entries) => entries.len(),
            RankingEntries::Float(entries) => entries.len(),
            RankingEntries::Playtime(entries) => entries.len(),
            RankingEntries::PpF32(entries) => entries.len(),
            RankingEntries::PpU32(entries) => entries.len(),
            RankingEntries::Rank(entries) => entries.len(),
        }
    }

    pub fn entry_count<R: RangeBounds<usize>>(&self, range: R) -> usize {
        match self {
            RankingEntries::Accuracy(entries) => entries.range(range).count(),
            RankingEntries::Amount(entries) => entries.range(range).count(),
            RankingEntries::AmountWithNegative(entries) => entries.range(range).count(),
            RankingEntries::Date(entries) => entries.range(range).count(),
            RankingEntries::Float(entries) => entries.range(range).count(),
            RankingEntries::Playtime(entries) => entries.range(range).count(),
            RankingEntries::PpF32(entries) => entries.range(range).count(),
            RankingEntries::PpU32(entries) => entries.range(range).count(),
            RankingEntries::Rank(entries) => entries.range(range).count(),
        }
    }

    pub fn name_pos(&self, name: &str) -> Option<usize> {
        match self {
            RankingEntries::Accuracy(entries) => {
                entries.values().position(|entry| entry.name == name)
            }
            RankingEntries::Amount(entries) => {
                entries.values().position(|entry| entry.name == name)
            }
            RankingEntries::AmountWithNegative(entries) => {
                entries.values().position(|entry| entry.name == name)
            }
            RankingEntries::Date(entries) => entries.values().position(|entry| entry.name == name),
            RankingEntries::Float(entries) => entries.values().position(|entry| entry.name == name),
            RankingEntries::Playtime(entries) => {
                entries.values().position(|entry| entry.name == name)
            }
            RankingEntries::PpF32(entries) => entries.values().position(|entry| entry.name == name),
            RankingEntries::PpU32(entries) => entries.values().position(|entry| entry.name == name),
            RankingEntries::Rank(entries) => entries.values().position(|entry| entry.name == name),
        }
    }
}

impl From<UserStatsEntries> for RankingEntries {
    #[inline]
    fn from(entries: UserStatsEntries) -> Self {
        match entries {
            UserStatsEntries::Accuracy(entries) => Self::Accuracy(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
            UserStatsEntries::Amount(entries) => Self::Amount(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
            UserStatsEntries::AmountWithNegative(entries) => Self::AmountWithNegative(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
            UserStatsEntries::Date(entries) => Self::Date(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
            UserStatsEntries::Float(entries) => Self::Float(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
            UserStatsEntries::Playtime(entries) => Self::Playtime(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
            UserStatsEntries::PpF32(entries) => Self::PpF32(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
            UserStatsEntries::Rank(entries) => Self::Rank(
                entries
                    .into_iter()
                    .map(RankingEntry::from)
                    .enumerate()
                    .collect(),
            ),
        }
    }
}

pub enum RankingKind {
    BgScores {
        global: bool,
        scores: Vec<BgGameScore>,
    },
    HlScores {
        scores: Vec<HlGameScore>,
        version: HlVersion,
    },
    OsekaiRarity,
    OsekaiMedalCount,
    OsekaiReplays,
    OsekaiTotalPp,
    OsekaiStandardDeviation,
    OsekaiBadges,
    OsekaiRankedMapsets,
    OsekaiLovedMapsets,
    OsekaiSubscribers,
    PpCountry {
        country: Box<str>,
        country_code: CountryCode,
        mode: GameMode,
    },
    PpGlobal {
        mode: GameMode,
    },
    RankedScore {
        mode: GameMode,
    },
    UserStats {
        guild_icon: Option<(Id<GuildMarker>, ImageHash)>,
        kind: UserStatsKind,
    },
}

pub enum EmbedHeader {
    Author(AuthorBuilder),
    Title { text: String, url: String },
}

impl EmbedHeader {
    fn title(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::Title {
            text: text.into(),
            url: url.into(),
        }
    }
}

impl RankingKind {
    pub fn embed_header(&self) -> EmbedHeader {
        match self {
            Self::BgScores { global, .. } => {
                let text = if *global {
                    "Global leaderboard for correct guesses"
                } else {
                    "Server leaderboard for correct guesses"
                };

                EmbedHeader::Author(AuthorBuilder::new(text))
            }
            Self::HlScores { version, .. } => {
                let text = match version {
                    HlVersion::ScorePp => "Server leaderboard for Higherlower (Score PP)",
                    HlVersion::FarmMaps => "Server leaderboard for Higherlower (Farm)",
                };

                EmbedHeader::Author(AuthorBuilder::new(text))
            }
            Self::OsekaiRarity => {
                let text = "Medal Ranking based on rarity";
                let url = "https://osekai.net/rankings/?ranking=Medals&type=Rarity";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiMedalCount => {
                let text = "User Ranking based on amount of owned medals";
                let url = "https://osekai.net/rankings/?ranking=Medals&type=Users";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiReplays => {
                let text = "User Ranking based on watched replays";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Replays";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiTotalPp => {
                let text = "User Ranking based on total pp across all modes";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Total+pp";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiStandardDeviation => {
                let text = "User Ranking based on pp standard deviation of all modes";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Standard+Deviation";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiBadges => {
                let text = "User Ranking based on amount of badges";
                let url = "https://osekai.net/rankings/?ranking=Badges&type=Badges";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiRankedMapsets => {
                let text = "User Ranking based on created ranked mapsets";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Ranked+Mapsets";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiLovedMapsets => {
                let text = "User Ranking based on created loved mapsets";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Loved+Mapsets";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiSubscribers => {
                let text = "User Ranking based on amount of mapping subscribers";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Subscribers";

                EmbedHeader::title(text, url)
            }
            Self::PpCountry {
                country,
                country_code,
                mode,
            } => {
                let text = format!(
                    "{country}'{plural} Performance Ranking for osu!{mode}",
                    plural = if country.ends_with('s') { "" } else { "s" },
                    mode = mode_str(*mode),
                );

                let url = format!(
                    "https://osu.ppy.sh/rankings/{mode}/performance?country={country_code}",
                );

                EmbedHeader::title(text, url)
            }
            Self::PpGlobal { mode } => {
                let text = format!("Performance Ranking for osu!{mode}", mode = mode_str(*mode));
                let url = format!("https://osu.ppy.sh/rankings/{mode}/performance");

                EmbedHeader::title(text, url)
            }
            Self::RankedScore { mode } => {
                let text = format!(
                    "Ranked Score Ranking for osu!{mode}",
                    mode = mode_str(*mode),
                );

                let url = format!("https://osu.ppy.sh/rankings/{mode}/score");

                EmbedHeader::title(text, url)
            }
            Self::UserStats { guild_icon, kind } => {
                let mut author_text = "Server leaderboard".to_owned();

                if let UserStatsKind::Mode { mode, .. } = kind {
                    let _ = write!(author_text, " for osu!{mode}", mode = mode_str(*mode));
                }

                let stats_kind = match kind {
                    UserStatsKind::AllModes { column } => match column {
                        UserStatsColumn::Badges => "Badges",
                        UserStatsColumn::Comments => "Comments",
                        UserStatsColumn::Followers => "Followers",
                        UserStatsColumn::ForumPosts => "Forum posts",
                        UserStatsColumn::GraveyardMapsets => "Graveyard mapsets",
                        UserStatsColumn::JoinDate => "Join date",
                        UserStatsColumn::KudosuAvailable => "Kudosu available",
                        UserStatsColumn::KudosuTotal => "Kudosu total",
                        UserStatsColumn::LovedMapsets => "Loved mapsets",
                        UserStatsColumn::Subscribers => "Mapping followers",
                        UserStatsColumn::Medals => "Medals",
                        UserStatsColumn::Namechanges => "Namechange count",
                        UserStatsColumn::PlayedMaps => "Played maps",
                        UserStatsColumn::RankedMapsets => "Ranked mapsets",
                    },
                    UserStatsKind::Mode { column, .. } => match column {
                        UserModeStatsColumn::Accuracy => "Accuracy",
                        UserModeStatsColumn::AverageHits => "Average hits per play",
                        UserModeStatsColumn::CountSsh => "Count SSH",
                        UserModeStatsColumn::CountSs => "Count SS",
                        UserModeStatsColumn::TotalSs => "Total SS",
                        UserModeStatsColumn::CountSh => "Count SH",
                        UserModeStatsColumn::CountS => "Count S",
                        UserModeStatsColumn::TotalS => "Total S",
                        UserModeStatsColumn::CountA => "Count A",
                        UserModeStatsColumn::Level => "Level",
                        UserModeStatsColumn::MaxCombo => "Max combo",
                        UserModeStatsColumn::Playcount => "Playcount",
                        UserModeStatsColumn::Playtime => "Playtime",
                        UserModeStatsColumn::Pp => "PP",
                        UserModeStatsColumn::PpPerMonth => "PP per month",
                        UserModeStatsColumn::RankCountry => "Country rank",
                        UserModeStatsColumn::RankGlobal => "Global rank",
                        UserModeStatsColumn::ReplaysWatched => "Replays watched",
                        UserModeStatsColumn::ScoreRanked => "Ranked score",
                        UserModeStatsColumn::ScoreTotal => "Total score",
                        UserModeStatsColumn::ScoresFirst => "Global #1s",
                        UserModeStatsColumn::Top1 => "Top PP",
                        UserModeStatsColumn::TotalHits => "Total hits",
                        UserModeStatsColumn::TopRange => "Top PP range",
                    },
                };

                let _ = write!(author_text, ": {stats_kind}");
                let mut author = AuthorBuilder::new(author_text);

                if let Some((id, icon)) = guild_icon {
                    let ext = if icon.animated { "gif" } else { "webp" };
                    let url = format!("https://cdn.discordapp.com/icons/{id}/{icon}.{ext}");
                    author = author.icon_url(url);
                }

                EmbedHeader::Author(author)
            }
        }
    }

    pub fn footer(
        &self,
        curr_page: usize,
        total_pages: usize,
        author_idx: Option<usize>,
    ) -> FooterBuilder {
        let mut text = format!("Page {curr_page}/{total_pages}");

        if let Some(idx) = author_idx {
            let _ = write!(text, " • Your position: {}", idx + 1);
        }

        match self {
            RankingKind::OsekaiRarity
            | RankingKind::OsekaiMedalCount
            | RankingKind::OsekaiReplays
            | RankingKind::OsekaiTotalPp
            | RankingKind::OsekaiStandardDeviation
            | RankingKind::OsekaiBadges
            | RankingKind::OsekaiRankedMapsets
            | RankingKind::OsekaiLovedMapsets
            | RankingKind::OsekaiSubscribers => {
                text.push_str(" • Check out osekai.net for more info")
            }
            _ => {}
        };

        FooterBuilder::new(text)
    }
}

pub enum UserStatsKind {
    AllModes {
        column: UserStatsColumn,
    },
    Mode {
        mode: GameMode,
        column: UserModeStatsColumn,
    },
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "",
        GameMode::Taiko => "taiko",
        GameMode::Catch => "ctb",
        GameMode::Mania => "mania",
    }
}

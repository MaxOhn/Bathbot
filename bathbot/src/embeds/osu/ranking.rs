use std::{
    collections::{btree_map::Range, BTreeMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
    ops::RangeBounds,
};

use bathbot_psql::model::{
    games::{DbBgGameScore, DbHlGameScore},
    osu::{UserModeStatsColumn, UserStatsColumn, UserStatsEntries, UserStatsEntry},
};
use rosu_v2::prelude::{GameMode, Username};
use time::OffsetDateTime;
use twilight_model::{
    channel::embed::Embed,
    id::{marker::GuildMarker, Id},
    util::ImageHash,
};

use crate::{
    embeds::EmbedData,
    games::hl::HlVersion,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, EmbedBuilder, FooterBuilder},
        numbers::{round, WithComma},
        rkyv_impls::ArchivedCountryCode,
        CountryCode,
    },
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
            country: Some(ArchivedCountryCode::new(entry.country).as_str().into()),
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

enum EmbedHeader {
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

pub enum RankingKind {
    BgScores {
        global: bool,
        scores: Vec<DbBgGameScore>,
    },
    HlScores {
        scores: Vec<DbHlGameScore>,
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
    UserStats {
        guild_icon: Option<(Id<GuildMarker>, ImageHash)>,
        kind: UserStatsKind,
    },
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

impl RankingKind {
    fn embed_header(&self) -> EmbedHeader {
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
                        UserModeStatsColumn::RankCountry => "Country rank",
                        UserModeStatsColumn::RankGlobal => "Global rank",
                        UserModeStatsColumn::ReplaysWatched => "Replays watched",
                        UserModeStatsColumn::ScoreRanked => "Ranked score",
                        UserModeStatsColumn::ScoreTotal => "Total score",
                        UserModeStatsColumn::ScoresFirst => "Global #1s",
                        UserModeStatsColumn::TotalHits => "Total hits",
                    },
                };

                let _ = write!(author_text, ": {stats_kind}");
                let mut author = AuthorBuilder::new(author_text);

                if let Some((id, icon)) = guild_icon {
                    let ext = if icon.is_animated() { "gif" } else { "webp" };
                    let url = format!("https://cdn.discordapp.com/icons/{id}/{icon}.{ext}");
                    author = author.icon_url(url);
                }

                EmbedHeader::Author(author)
            }
        }
    }

    fn footer(
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

pub struct RankingEmbed {
    description: String,
    footer: FooterBuilder,
    header: EmbedHeader,
}

impl RankingEmbed {
    pub fn new(
        entries: &RankingEntries,
        kind: &RankingKind,
        author_idx: Option<usize>,
        pages: &Pages,
    ) -> Self {
        let page = pages.curr_page();
        let pages = pages.last_page();
        let idx = (page - 1) * 20;
        let mut buf = String::new();
        let description = String::with_capacity(1024);
        let footer = kind.footer(page, pages, author_idx);
        let header = kind.embed_header();

        match entries {
            RankingEntries::Accuracy(entries) => Self::finalize::<_, Accuracy<'_>>(
                &mut buf,
                description,
                entries,
                idx,
                footer,
                header,
            ),
            RankingEntries::Amount(entries) => {
                Self::finalize::<_, Amount<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::AmountWithNegative(entries) => {
                Self::finalize::<_, AmountWithNegative<'_>>(
                    &mut buf,
                    description,
                    entries,
                    idx,
                    footer,
                    header,
                )
            }
            RankingEntries::Date(entries) => {
                Self::finalize::<_, Date<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::Float(entries) => {
                Self::finalize::<_, Float<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::Playtime(entries) => Self::finalize::<_, Playtime<'_>>(
                &mut buf,
                description,
                entries,
                idx,
                footer,
                header,
            ),
            RankingEntries::PpF32(entries) => {
                Self::finalize::<_, PpF32<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::PpU32(entries) => {
                Self::finalize::<_, PpU32<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::Rank(entries) => {
                Self::finalize::<_, Rank<'_>>(&mut buf, description, entries, idx, footer, header)
            }
        }
    }

    fn finalize<'v, V, F>(
        buf: &mut String,
        mut description: String,
        entries: &'v BTreeMap<usize, RankingEntry<V>>,
        idx: usize,
        footer: FooterBuilder,
        header: EmbedHeader,
    ) -> Self
    where
        F: From<&'v V> + Display,
        V: 'v,
    {
        let left_lengths = Lengths::new::<V, F>(buf, entries.range(idx..idx + 10));
        let right_lengths = Lengths::new::<V, F>(buf, entries.range(idx + 10..idx + 20));

        // Ensuring the right side has ten elements for the zip
        let user_iter = entries
            .range(idx..idx + 10)
            .zip((10..20).map(|i| entries.get(&(idx + i))));

        for ((i, left_entry), right) in user_iter {
            let idx = i + 1;

            buf.clear();
            let _ = write!(buf, "{}", F::from(&left_entry.value));

            let _ = write!(
                description,
                "`#{idx:<idx_len$}`{country}`{name:<name_len$}` `{buf:>value_len$}`",
                idx_len = left_lengths.idx,
                country = CountryFormatter::new(left_entry),
                name = left_entry.name,
                name_len = left_lengths.name,
                value_len = left_lengths.value,
            );

            if let Some(right_entry) = right {
                buf.clear();
                let _ = write!(buf, "{}", F::from(&right_entry.value));

                let _ = write!(
                    description,
                    "|`#{idx:<idx_len$}`{country}`{name:<name_len$}` `{buf:>value_len$}`",
                    idx = idx + 10,
                    idx_len = right_lengths.idx,
                    country = CountryFormatter::new(right_entry),
                    name = right_entry.name,
                    name_len = right_lengths.name,
                    value_len = right_lengths.value,
                );
            }

            description.push('\n');
        }

        Self {
            description,
            footer,
            header,
        }
    }
}

impl EmbedData for RankingEmbed {
    fn build(self) -> Embed {
        let builder = EmbedBuilder::new()
            .description(self.description)
            .footer(self.footer);

        match self.header {
            EmbedHeader::Author(author) => builder.author(author).build(),
            EmbedHeader::Title { text, url } => builder.title(text).url(url).build(),
        }
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

struct Lengths {
    idx: usize,
    name: usize,
    value: usize,
}

impl Lengths {
    fn new<'v, V, F>(buf: &mut String, iter: Range<'v, usize, RankingEntry<V>>) -> Self
    where
        F: From<&'v V> + Display,
        V: 'v,
    {
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
            name_len = name_len.max(entry.name.chars().count());

            buf.clear();
            let _ = write!(buf, "{}", F::from(&entry.value));
            value_len = value_len.max(buf.len());
        }

        Lengths {
            idx: idx_len,
            name: name_len,
            value: value_len,
        }
    }
}

struct CountryFormatter<'e, V> {
    entry: &'e RankingEntry<V>,
}

impl<'e, V> CountryFormatter<'e, V> {
    fn new(entry: &'e RankingEntry<V>) -> Self {
        Self { entry }
    }
}

impl<V> Display for CountryFormatter<'_, V> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if let Some(ref country) = self.entry.country {
            write!(f, ":flag_{}:", country.to_ascii_lowercase())
        } else {
            f.write_str(" ")
        }
    }
}

macro_rules! formatter {
    ( $( $name:ident<$ty:ident> ,)* ) => {
        $(
            struct $name<'i> {
                inner: &'i $ty,
            }

            impl<'i> From<&'i $ty> for $name<'i> {
                #[inline]
                fn from(value: &'i $ty) -> Self {
                    Self { inner: value }
                }
            }
        )*
    };
}

formatter! {
    Accuracy<f32>,
    Amount<u64>,
    AmountWithNegative<i64>,
    Date<OffsetDateTime>,
    Float<f32>,
    Playtime<u32>,
    PpF32<f32>,
    PpU32<u32>,
    Rank<u32>,
}

impl Display for Accuracy<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:.2}%", self.inner)
    }
}

impl Display for Amount<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", AmountWithNegative::from(&(*self.inner as i64)))
    }
}

impl Display for AmountWithNegative<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.inner.abs() < 1_000_000_000 {
            write!(f, "{}", WithComma::new(*self.inner))
        } else {
            let score = (self.inner / 10_000_000) as f32 / 100.0;

            write!(f, "{score:.2} bn")
        }
    }
}

impl Display for Date<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.inner.date())
    }
}

impl Display for Float<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:.2}", self.inner)
    }
}

impl Display for Playtime<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} hrs", WithComma::new(self.inner / 60 / 60))
    }
}

impl Display for PpF32<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}pp", WithComma::new(round(*self.inner)))
    }
}

impl Display for PpU32<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}pp", WithComma::new(*self.inner))
    }
}

impl Display for Rank<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "#{}", WithComma::new(*self.inner))
    }
}

use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_macros::EmbedData;
use bathbot_model::{rosu_v2::user::User, OsuTrackerMapsetEntry};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, FooterBuilder, IntHasher,
};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;

use crate::{
    commands::osu::{TopEntry, TopScoreOrder},
    manager::{redis::RedisData, OsuMap},
    pagination::Pages,
    util::{osu::grade_emote, Emote},
};

use super::{ComboFormatter, HitResultFormatter, ModsFormatter, PpFormatter};

type Farm = HashMap<u32, (OsuTrackerMapsetEntry, bool), IntHasher>;

#[derive(EmbedData)]
pub struct TopEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl TopEmbed {
    pub fn new(
        user: &RedisData<User>,
        entries: &[TopEntry],
        sort_by: impl Into<TopScoreOrder>,
        farm: &Farm,
        mode: GameMode,
        pages: &Pages,
    ) -> Self {
        Self::new_(user, entries, sort_by.into(), farm, mode, pages)
    }

    pub fn new_(
        user: &RedisData<User>,
        entries: &[TopEntry],
        sort_by: TopScoreOrder,
        farm: &Farm,
        mode: GameMode,
        pages: &Pages,
    ) -> Self {
        let mut description = String::with_capacity(512);

        for entry in entries {
            let TopEntry {
                original_idx,
                score,
                map,
                max_pp,
                max_combo,
                stars,
            } = entry;

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {grade} {pp} • {acc}% • {score}\n[ {combo} ] • {hits} • {appendix}",
                idx = *original_idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = map.map_id(),
                mods = ModsFormatter::new(score.mods),
                grade = grade_emote(score.grade),
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, Some(*max_combo)),
                hits = HitResultFormatter::new(score.mode, score.statistics.clone()),
                appendix = OrderAppendix::new(sort_by, entry, map.ranked_date(), farm, false),
            );
        }

        description.pop();

        let footer_text = format!(
            "Page {}/{} | Mode: {}",
            pages.curr_page(),
            pages.last_page(),
            mode_str(mode)
        );

        Self {
            author: user.author_builder(),
            description,
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url().to_owned(),
        }
    }
}

#[derive(EmbedData)]
pub struct CondensedTopEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl CondensedTopEmbed {
    pub fn new(
        user: &RedisData<User>,
        entries: &[TopEntry],
        sort_by: TopScoreOrder,
        farm: &Farm,
        mode: GameMode,
        pages: &Pages,
    ) -> Self {
        let description = if mode == GameMode::Mania {
            Self::description_mania(entries, sort_by, farm)
        } else {
            Self::description(entries, sort_by, farm)
        };

        let footer_text = format!(
            "Page {}/{} • Mode: {}",
            pages.curr_page(),
            pages.last_page(),
            mode_str(mode)
        );

        Self {
            author: user.author_builder(),
            description,
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url().to_owned(),
        }
    }

    fn description(entries: &[TopEntry], sort_by: TopScoreOrder, farm: &Farm) -> String {
        let mut description = String::with_capacity(1024);

        for entry in entries {
            let TopEntry {
                original_idx,
                score,
                map,
                stars,
                max_pp: _,
                max_combo,
            } = entry;

            let _ = writeln!(
                description,
                "**{idx}. [{map}]({OSU_BASE}b/{map_id})** [{stars}★]\n\
                {grade} **{pp}pp** ({acc}%) [**{combo}x**/{max_combo}x] {miss}**+{mods}** {appendix}",
                idx = *original_idx + 1,
                map = MapFormat::new(map),
                map_id = map.map_id(),
                stars = round(*stars),
                grade = grade_emote(score.grade),
                pp = round(score.pp),
                acc = round(score.accuracy),
                combo = score.max_combo,
                miss = MissFormat(score.statistics.count_miss),
                mods = score.mods,
                appendix = OrderAppendix::new(sort_by, entry, map.ranked_date(), farm, true),
            );
        }

        description
    }

    fn description_mania(entries: &[TopEntry], sort_by: TopScoreOrder, farm: &Farm) -> String {
        let mut description = String::with_capacity(1024);

        for entry in entries {
            let TopEntry {
                original_idx,
                score,
                map,
                max_pp: _,
                max_combo: _,
                stars: _,
            } = entry;

            let stats = &score.statistics;

            let _ = writeln!(
                description,
                "**{idx}. [{map}]({OSU_BASE}b/{map_id}) +{mods}**\n\
                {grade} **{pp}pp** ({acc}%) `{score}` {{{n320}/{n300}/.../{miss}}} {appendix}",
                idx = *original_idx + 1,
                map = MapFormat::new(map),
                map_id = map.map_id(),
                mods = score.mods,
                grade = grade_emote(score.grade),
                pp = round(score.pp),
                acc = round(score.accuracy),
                score = ScoreFormat(score.score),
                n320 = stats.count_geki,
                n300 = stats.count_300,
                miss = stats.count_miss,
                appendix = OrderAppendix::new(sort_by, entry, map.ranked_date(), farm, true),
            );
        }

        description
    }
}

struct MapFormat<'m> {
    map: &'m OsuMap,
}

impl<'m> MapFormat<'m> {
    fn new(map: &'m OsuMap) -> Self {
        Self { map }
    }
}

impl Display for MapFormat<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        const DASH_SPACE_SQUARE_BRACKETS: usize = 6;
        const SPACE_SQUARE_BRACKETS: usize = 3;
        const TRIPLE_DOTS: usize = 3;
        const MIN_LEN: usize = 15;
        const LIMIT: usize = 42;
        const MIN_LEN_WITH_DOTS: usize = MIN_LEN + TRIPLE_DOTS;

        let artist = self.map.artist().len();
        let title = self.map.title().len();
        let version = self.map.version().len();

        // if the dots wouldn't save space, might as well not replace the content
        let tuple = |pre, post| {
            if pre <= post + TRIPLE_DOTS {
                (pre, "")
            } else {
                (post, "...")
            }
        };

        if artist + title + version + DASH_SPACE_SQUARE_BRACKETS <= LIMIT {
            // short enough to display artist, title, and version
            write!(
                f,
                "{} - {} [{}]",
                self.map.artist().cow_escape_markdown(),
                self.map.title().cow_escape_markdown(),
                self.map.version().cow_escape_markdown(),
            )
        } else if title + version + SPACE_SQUARE_BRACKETS <= LIMIT {
            // show title and version without truncating
            write!(
                f,
                "{} [{}]",
                self.map.title().cow_escape_markdown(),
                self.map.version().cow_escape_markdown()
            )
        } else if version <= MIN_LEN_WITH_DOTS {
            // keep the version but truncate title
            let (end, suffix) = tuple(title, LIMIT - version - SPACE_SQUARE_BRACKETS - TRIPLE_DOTS);

            write!(
                f,
                "{}{suffix} [{}]",
                self.map.title()[..end].cow_escape_markdown(),
                self.map.version().cow_escape_markdown(),
            )
        } else if title <= MIN_LEN_WITH_DOTS {
            // keep the title but truncate version
            let (end, suffix) = tuple(version, LIMIT - title - SPACE_SQUARE_BRACKETS - TRIPLE_DOTS);

            write!(
                f,
                "{} [{}{suffix}]",
                self.map.title().cow_escape_markdown(),
                self.map.version()[..end].cow_escape_markdown(),
            )
        } else {
            // truncate title and version evenly
            let total_cut = title + version + SPACE_SQUARE_BRACKETS - LIMIT;
            let mut cut = total_cut / 2;

            let mut title_cut = match title.checked_sub(cut) {
                Some(len @ ..=MIN_LEN) => {
                    cut += MIN_LEN - len;

                    MIN_LEN
                }
                Some(len @ ..=MIN_LEN_WITH_DOTS) => {
                    cut += MIN_LEN_WITH_DOTS - len;

                    MIN_LEN
                }
                Some(len) => len - TRIPLE_DOTS,
                None => {
                    cut += cut - MIN_LEN;

                    MIN_LEN
                }
            };

            // if cut was off, increment by one
            cut += total_cut % 2;

            let version_cut = match version.checked_sub(cut) {
                Some(len @ ..=MIN_LEN) => {
                    title_cut -= MIN_LEN - len;

                    MIN_LEN
                }
                Some(len @ ..=MIN_LEN_WITH_DOTS) => {
                    title_cut -= MIN_LEN_WITH_DOTS - len;

                    MIN_LEN
                }
                Some(len) => len - TRIPLE_DOTS,
                None => {
                    title_cut -= cut - MIN_LEN;

                    MIN_LEN
                }
            };

            let (title_end, title_suffix) = tuple(title, title_cut);
            let (version_end, version_suffix) = tuple(version, version_cut);

            write!(
                f,
                "{}{title_suffix} [{}{version_suffix}]",
                self.map.title()[..title_end].cow_escape_markdown(),
                self.map.version()[..version_end].cow_escape_markdown(),
            )
        }
    }
}

struct MissFormat(u32);

impl Display for MissFormat {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == 0 {
            return Ok(());
        }

        write!(
            f,
            "{miss}{emote} ",
            miss = self.0,
            emote = Emote::Miss.text()
        )
    }
}

struct ScoreFormat(u32);

impl Display for ScoreFormat {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 < 10_000 {
            write!(f, "{}", self.0)
        } else {
            write!(f, "{}K", self.0 / 1000)
        }
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "osu!",
        GameMode::Taiko => "Taiko",
        GameMode::Catch => "Catch",
        GameMode::Mania => "Mania",
    }
}

pub struct OrderAppendix<'a> {
    sort_by: TopScoreOrder,
    entry: &'a TopEntry,
    ranked_date: Option<OffsetDateTime>,
    farm: &'a Farm,
    condensed: bool,
}

impl<'a> OrderAppendix<'a> {
    pub fn new(
        sort_by: TopScoreOrder,
        entry: &'a TopEntry,
        ranked_date: Option<OffsetDateTime>,
        farm: &'a Farm,
        condensed: bool,
    ) -> Self {
        Self {
            sort_by,
            entry,
            ranked_date,
            farm,
            condensed,
        }
    }
}

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort_by {
            TopScoreOrder::Farm => {
                let mapset_id = self.entry.map.mapset_id();

                let count = self
                    .farm
                    .get(&mapset_id)
                    .map_or(0, |(entry, _)| entry.count);

                write!(f, "`{}`", WithComma::new(count))
            }
            TopScoreOrder::Bpm => {
                let clock_rate = self.entry.score.mods.clock_rate();

                write!(f, "`{}bpm`", round(self.entry.map.bpm() * clock_rate))
            }
            TopScoreOrder::Length => {
                let clock_rate = self.entry.score.mods.clock_rate();

                let secs = (self.entry.map.seconds_drain() as f32 / clock_rate) as u32;

                write!(f, "`{}:{:0>2}`", secs / 60, secs % 60)
            }
            TopScoreOrder::RankedDate => match self.ranked_date {
                Some(date) => write!(f, "<t:{}:d>", date.unix_timestamp()),
                None => Ok(()),
            },
            TopScoreOrder::Score if self.condensed && self.entry.map.mode() != GameMode::Mania => {
                let score = self.entry.score.score;

                if score > 1_000_000_000 {
                    write!(f, "`{:.2}bn`", score as f32 / 1_000_000_000.0)
                } else if score > 1_000_000 {
                    write!(f, "`{:.2}m`", score as f32 / 1_000_000.0)
                } else {
                    write!(f, "`{}`", WithComma::new(score))
                }
            }
            _ => write!(f, "{}", HowLongAgoDynamic::new(&self.entry.score.ended_at)),
        }
    }
}

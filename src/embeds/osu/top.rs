use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use command_macros::EmbedData;
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;

use crate::{
    commands::osu::{TopEntry, TopScoreOrder},
    custom_client::OsuTrackerMapsetEntry,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::HowLongAgoDynamic,
        hasher::IntHasher,
        numbers::{round, WithComma},
        osu::grade_emote,
        CowUtils, Emote,
    },
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
        pages: &Pages,
    ) -> Self {
        Self::new_(user, entries, sort_by.into(), farm, pages)
    }

    pub fn new_(
        user: &RedisData<User>,
        entries: &[TopEntry],
        sort_by: TopScoreOrder,
        farm: &Farm,
        pages: &Pages,
    ) -> Self {
        let mut description = String::with_capacity(512);

        for entry in entries {
            let TopEntry {
                original_idx,
                score,
                map,
                max_pp,
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
                acc = score.accuracy,
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, map.max_combo()),
                hits = HitResultFormatter::new(score.mode, score.statistics.clone()),
                appendix = OrderAppendix::new(sort_by, entry, map.ranked_date(), farm, false),
            );
        }

        description.pop();

        let footer_text = format!(
            "Page {}/{} | Mode: {}",
            pages.curr_page(),
            pages.last_page(),
            mode_str(user.mode())
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
        pages: &Pages,
    ) -> Self {
        let mode = user.mode();

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
                max_combo = map.max_combo().unwrap_or(0),
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
        let artist = self.map.artist().len();
        let title = self.map.title().len();
        let version = self.map.version().len();

        const LIMIT: usize = 46;

        // if the dots wouldn't save space, might as well not replace the content
        let tuple = |pre, post| {
            if pre <= post + 3 {
                (pre, "")
            } else {
                (post, "...")
            }
        };

        if artist + title + version + 6 <= LIMIT {
            // short enough to display artist, title, and version
            write!(
                f,
                "{} - {} [{}]",
                self.map.artist().cow_escape_markdown(),
                self.map.title().cow_escape_markdown(),
                self.map.version().cow_escape_markdown(),
            )
        } else if title + version + 3 <= LIMIT {
            // show title and version without truncating
            write!(
                f,
                "{} [{}]",
                self.map.title().cow_escape_markdown(),
                self.map.version().cow_escape_markdown()
            )
        } else if version < 15 {
            // keep the version but truncate title
            let (end, suffix) = tuple(title, 50 - version - 3 - 3);

            write!(
                f,
                "{}{suffix} [{}]",
                self.map.title()[..end].cow_escape_markdown(),
                self.map.version().cow_escape_markdown(),
            )
        } else if title < 15 {
            // keep the title but truncate version
            let (end, suffix) = tuple(version, 50 - title - 3 - 3);

            write!(
                f,
                "{} [{}{suffix}]",
                self.map.title().cow_escape_markdown(),
                self.map.version()[..end].cow_escape_markdown(),
            )
        } else {
            // truncate title and version evenly
            let cut = (title + version + 3 + 6 - LIMIT) / 2;

            let mut title_ = title.saturating_sub(cut).max(15);
            let mut version_ = version.saturating_sub(cut).max(15);

            if title_ + version_ + 3 > LIMIT - 6 {
                if title_ == 15 {
                    version_ = LIMIT - title_ - 3 - 6;
                } else if version_ == 15 {
                    title_ = LIMIT - version_ - 3 - 6;
                }
            }

            let (title_end, title_suffix) = tuple(title, title_);
            let (version_end, version_suffix) = tuple(version, version_);

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

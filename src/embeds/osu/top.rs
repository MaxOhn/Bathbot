use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use command_macros::EmbedData;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, Beatmapset, BeatmapsetCompact, GameMode, GameMods, Score, User};
use time::OffsetDateTime;

use crate::{
    commands::osu::TopScoreOrder,
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    embeds::osu,
    pagination::Pages,
    pp::PpCalculator,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        hasher::SimpleBuildHasher,
        numbers::{round, with_comma_int},
        CowUtils, Emote, ScoreExt,
    },
};

type Farm = HashMap<u32, (OsuTrackerMapsetEntry, bool), SimpleBuildHasher>;

#[derive(EmbedData)]
pub struct TopEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl TopEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: impl Into<TopScoreOrder>,
        farm: &Farm,
        pages: &Pages,
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        Self::new_(user, scores, ctx, sort_by.into(), farm, pages).await
    }

    pub async fn new_<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: TopScoreOrder,
        farm: &Farm,
        pages: &Pages,
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(512);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let (pp, max_pp, stars) = match PpCalculator::new(ctx, map.map_id).await {
                Ok(mut calc) => {
                    calc.score(score);

                    let stars = calc.stars();
                    let max_pp = calc.max_pp();

                    let pp = match score.pp {
                        Some(pp) => pp,
                        None => calc.pp() as f32,
                    };

                    (Some(pp), Some(max_pp as f32), stars as f32)
                }
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    (None, None, 0.0)
                }
            };

            let stars = osu::get_stars(stars);
            let pp = osu::get_pp(pp, max_pp);

            let mapset_opt = if let TopScoreOrder::RankedDate = sort_by {
                retrieve_mapset(ctx, mapset.mapset_id).await
            } else {
                None
            };

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} • {acc}% • {score}\n[ {combo} ] • {hits} • {appendix}",
                idx = idx + 1,
                title = mapset.title.cow_escape_markdown(),
                version = map.version.cow_escape_markdown(),
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                grade = score.grade_emote(score.mode),
                acc = score.acc(score.mode),
                score = with_comma_int(score.score),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(score.mode),
                appendix = OrderAppendix::new(sort_by, map, mapset_opt, score, farm, false),
            );
        }

        description.pop();

        let footer_text = format!(
            "Page {}/{} | Mode: {}",
            pages.curr_page(),
            pages.last_page(),
            mode_str(user.mode)
        );

        Self {
            author: author!(user),
            description,
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url.to_owned(),
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
    pub async fn new<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: TopScoreOrder,
        farm: &Farm,
        pages: &Pages,
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let description = if user.mode == GameMode::Mania {
            Self::description_mania(scores, ctx, sort_by, farm).await
        } else {
            Self::description(scores, ctx, sort_by, farm).await
        };

        let footer_text = format!(
            "Page {}/{} • Mode: {}",
            pages.curr_page(),
            pages.last_page(),
            mode_str(user.mode)
        );

        Self {
            author: author!(user),
            description,
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url.to_owned(),
        }
    }

    async fn description<'i, S>(
        scores: S,
        ctx: &Context,
        sort_by: TopScoreOrder,
        farm: &Farm,
    ) -> String
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(1024);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let (pp, stars) = match PpCalculator::new(ctx, map.map_id).await {
                Ok(mut calc) => {
                    calc.score(score);
                    let stars = calc.stars();

                    let pp = match score.pp {
                        Some(pp) => pp,
                        None => calc.pp() as f32,
                    };

                    (pp, stars as f32)
                }
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    (0.0, 0.0)
                }
            };

            let mapset_opt = if let TopScoreOrder::RankedDate = sort_by {
                retrieve_mapset(ctx, mapset.mapset_id).await
            } else {
                None
            };

            let _ = writeln!(
                description,
                "**{idx}. [{map}]({OSU_BASE}b/{map_id})** [{stars}★]\n\
                {grade} **{pp}pp** ({acc}%) [**{combo}x**/{max_combo}x] {miss}**+{mods}** {appendix}",
                idx = idx + 1,
                map = MapFormat { map, mapset },
                map_id = map.map_id,
                stars = round(stars),
                grade = score.grade_emote(score.mode),
                pp = round(pp),
                acc = round(score.accuracy),
                combo = score.max_combo,
                max_combo = map.max_combo.unwrap_or(0),
                miss = MissFormat(score.statistics.count_miss),
                mods = score.mods,
                appendix = OrderAppendix::new(sort_by, map, mapset_opt, score, farm, true),
            );
        }

        description
    }

    async fn description_mania<'i, S>(
        scores: S,
        ctx: &Context,
        sort_by: TopScoreOrder,
        farm: &Farm,
    ) -> String
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(1024);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let pp = match score.pp {
                Some(pp) => pp,
                None => match PpCalculator::new(ctx, map.map_id).await {
                    Ok(mut calc) => calc.pp() as f32,
                    Err(err) => {
                        warn!("{:?}", Report::new(err));

                        0.0
                    }
                },
            };

            let stats = &score.statistics;

            let mapset_opt = if let TopScoreOrder::RankedDate = sort_by {
                retrieve_mapset(ctx, mapset.mapset_id).await
            } else {
                None
            };

            let _ = writeln!(
                description,
                "**{idx}. [{map}]({OSU_BASE}b/{map_id}) +{mods}**\n\
                {grade} **{pp}pp** ({acc}%) `{score}` {{{n320}/{n300}/.../{miss}}} {appendix}",
                idx = idx + 1,
                map = MapFormat { map, mapset },
                map_id = map.map_id,
                mods = score.mods,
                grade = score.grade_emote(score.mode),
                pp = round(pp),
                acc = round(score.accuracy),
                score = ScoreFormat(score.score),
                n320 = stats.count_geki,
                n300 = stats.count_300,
                miss = stats.count_miss,
                appendix = OrderAppendix::new(sort_by, map, mapset_opt, score, farm, true),
            );
        }

        description
    }
}

struct MapFormat<'m> {
    map: &'m Beatmap,
    mapset: &'m BeatmapsetCompact,
}

impl Display for MapFormat<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let artist = self.mapset.artist.len();
        let title = self.mapset.title.len();
        let version = self.map.version.len();

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
                self.mapset.artist.cow_escape_markdown(),
                self.mapset.title.cow_escape_markdown(),
                self.map.version.cow_escape_markdown(),
            )
        } else if title + version + 3 <= LIMIT {
            // show title and version without truncating
            write!(
                f,
                "{} [{}]",
                self.mapset.title.cow_escape_markdown(),
                self.map.version.cow_escape_markdown()
            )
        } else if version < 15 {
            // keep the version but truncate title
            let (end, suffix) = tuple(title, 50 - version - 3 - 3);

            write!(
                f,
                "{}{suffix} [{}]",
                self.mapset.title[..end].cow_escape_markdown(),
                self.map.version.cow_escape_markdown(),
            )
        } else if title < 15 {
            // keep the title but truncate version
            let (end, suffix) = tuple(version, 50 - title - 3 - 3);

            write!(
                f,
                "{} [{}{suffix}]",
                self.mapset.title.cow_escape_markdown(),
                self.map.version[..end].cow_escape_markdown(),
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
                self.mapset.title[..title_end].cow_escape_markdown(),
                self.map.version[..version_end].cow_escape_markdown(),
            )
        }
    }
}

struct MissFormat(u32);

impl Display for MissFormat {
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
    map: &'a Beatmap,
    ranked_date: Option<OffsetDateTime>,
    score: &'a Score,
    farm: &'a Farm,
    condensed: bool,
}

impl<'a> OrderAppendix<'a> {
    pub fn new(
        sort_by: TopScoreOrder,
        map: &'a Beatmap,
        mapset: Option<Beatmapset>,
        score: &'a Score,
        farm: &'a Farm,
        condensed: bool,
    ) -> Self {
        let ranked_date = mapset.and_then(|mapset| mapset.ranked_date);

        Self {
            sort_by,
            map,
            ranked_date,
            score,
            farm,
            condensed,
        }
    }
}

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort_by {
            TopScoreOrder::Farm => {
                let mapset_id = self.map.mapset_id;
                let count = self
                    .farm
                    .get(&mapset_id)
                    .map_or(0, |(entry, _)| entry.count);

                write!(f, "`{}`", with_comma_int(count))
            }
            TopScoreOrder::Bpm => {
                let mods = self.score.mods;

                let clock_rate = if mods.contains(GameMods::DoubleTime) {
                    1.5
                } else if mods.contains(GameMods::HalfTime) {
                    0.75
                } else {
                    1.0
                };

                write!(f, "`{}bpm`", round(self.map.bpm * clock_rate))
            }
            TopScoreOrder::Length => {
                let mods = self.score.mods;

                let clock_rate = if mods.contains(GameMods::DoubleTime) {
                    1.5
                } else if mods.contains(GameMods::HalfTime) {
                    0.75
                } else {
                    1.0
                };

                let secs = (self.map.seconds_drain as f32 / clock_rate) as u32;

                write!(f, "`{}:{:0>2}`", secs / 60, secs % 60)
            }
            TopScoreOrder::RankedDate => match self.ranked_date {
                Some(date) => write!(f, "<t:{}:d>", date.unix_timestamp()),
                None => Ok(()),
            },
            TopScoreOrder::Score if self.condensed && self.map.mode != GameMode::Mania => {
                let score = self.score.score;

                if score > 1_000_000_000 {
                    write!(f, "`{:.2}bn`", score as f32 / 1_000_000_000.0)
                } else if score > 1_000_000 {
                    write!(f, "`{:.2}m`", score as f32 / 1_000_000.0)
                } else {
                    write!(f, "`{}`", with_comma_int(score))
                }
            }
            _ => write!(f, "<t:{}:R>", self.score.ended_at.unix_timestamp()),
        }
    }
}

async fn retrieve_mapset(ctx: &Context, mapset_id: u32) -> Option<Beatmapset> {
    let mapset_fut = ctx.psql().get_beatmapset::<Beatmapset>(mapset_id);

    match mapset_fut.await {
        Ok(mapset) => {
            if let Err(err) = ctx.psql().insert_beatmapset(&mapset).await {
                let report = Report::new(err).wrap_err("failed to insert mapset into DB");
                warn!("{report:?}");
            }

            Some(mapset)
        }
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to get mapset");
            warn!("{report:?}");

            None
        }
    }
}

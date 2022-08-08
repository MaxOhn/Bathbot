use std::{borrow::Cow, fmt::Write};

use rosu_pp::{Beatmap as Map, BeatmapExt, DifficultyAttributes};
use rosu_v2::prelude::{
    Beatmap, BeatmapsetCompact, GameMode, GameMods, Grade, Score, ScoreStatistics,
};
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{
        HasMods, ModsResult, RecentSimulateCatch, RecentSimulateMania, RecentSimulateOsu,
        RecentSimulateTaiko,
    },
    core::Context,
    embeds::osu,
    error::PpError,
    pp::PpCalculator,
    util::{
        builder::{EmbedBuilder, FooterBuilder},
        constants::{AVATAR_URL, MAP_THUMB_URL},
        numbers::{round, with_comma_int},
        osu::{grade_completion_mods, prepare_beatmap_file, ModSelection},
        CowUtils, ScoreExt,
    },
    BotResult,
};

pub struct SimulateArgs {
    mods: Option<ModSelection>,
    n300: Option<usize>,
    n100: Option<usize>,
    n50: Option<usize>,
    misses: Option<usize>,
    acc: Option<f32>,
    combo: Option<usize>,
    score: Option<u32>,
}

impl SimulateArgs {
    fn is_some(&self) -> bool {
        self.mods.is_some()
            || self.n300.is_some()
            || self.n100.is_some()
            || self.n50.is_some()
            || self.misses.is_some()
            || self.acc.is_some()
            || self.combo.is_some()
            || self.score.is_some()
    }
}

static ERR_PARSE_MODS: &str = "Failed to parse mods. Be sure to either specify them directly \
    or through the `+mods` / `+mods!` syntax e.g. `hdhr` or `+hdhr!`";

impl TryFrom<RecentSimulateOsu<'_>> for SimulateArgs {
    type Error = &'static str;

    fn try_from(args: RecentSimulateOsu<'_>) -> Result<Self, Self::Error> {
        let mods = match args.mods() {
            ModsResult::Mods(mods) => Some(mods),
            ModsResult::None => None,
            ModsResult::Invalid => return Err(ERR_PARSE_MODS),
        };

        Ok(Self {
            mods,
            n300: args.n300.map(|n| n as usize),
            n100: args.n100.map(|n| n as usize),
            n50: args.n50.map(|n| n as usize),
            misses: args.misses.map(|n| n as usize),
            acc: args.acc,
            combo: args.combo.map(|n| n as usize),
            score: None,
        })
    }
}

impl TryFrom<RecentSimulateTaiko<'_>> for SimulateArgs {
    type Error = &'static str;

    fn try_from(args: RecentSimulateTaiko<'_>) -> Result<Self, Self::Error> {
        let mods = match args.mods() {
            ModsResult::Mods(mods) => Some(mods),
            ModsResult::None => None,
            ModsResult::Invalid => return Err(ERR_PARSE_MODS),
        };

        Ok(Self {
            mods,
            n300: args.n300.map(|n| n as usize),
            n100: args.n100.map(|n| n as usize),
            n50: None,
            misses: args.misses.map(|n| n as usize),
            acc: args.acc,
            combo: args.combo.map(|n| n as usize),
            score: None,
        })
    }
}

impl TryFrom<RecentSimulateCatch<'_>> for SimulateArgs {
    type Error = &'static str;

    fn try_from(args: RecentSimulateCatch<'_>) -> Result<Self, Self::Error> {
        let mods = match args.mods() {
            ModsResult::Mods(mods) => Some(mods),
            ModsResult::None => None,
            ModsResult::Invalid => return Err(ERR_PARSE_MODS),
        };

        Ok(Self {
            mods,
            n300: args.fruits.map(|n| n as usize),
            n100: args.droplets.map(|n| n as usize),
            n50: args.tiny_droplets.map(|n| n as usize),
            misses: args.misses.map(|n| n as usize),
            acc: args.acc,
            combo: args.combo.map(|n| n as usize),
            score: None,
        })
    }
}

impl TryFrom<RecentSimulateMania<'_>> for SimulateArgs {
    type Error = &'static str;

    fn try_from(args: RecentSimulateMania<'_>) -> Result<Self, Self::Error> {
        let mods = match args.mods() {
            ModsResult::Mods(mods) => Some(mods),
            ModsResult::None => None,
            ModsResult::Invalid => return Err(ERR_PARSE_MODS),
        };

        Ok(Self {
            mods,
            n300: None,
            n100: None,
            n50: None,
            misses: None,
            acc: None,
            combo: None,
            score: args.score,
        })
    }
}

impl From<crate::commands::osu::SimulateArgs> for SimulateArgs {
    fn from(args: crate::commands::osu::SimulateArgs) -> Self {
        Self {
            mods: args.mods,
            n300: args.n300,
            n100: args.n100,
            n50: args.n50,
            misses: args.misses,
            acc: args.acc,
            combo: args.combo,
            score: args.score,
        }
    }
}

pub struct SimulateEmbed {
    title: String,
    url: String,
    footer: FooterBuilder,
    thumbnail: String,

    mode: GameMode,
    stars: f32,
    grade_completion_mods: Cow<'static, str>,
    acc: f32,
    prev_pp: Option<f32>,
    pp: String,
    prev_combo: Option<u32>,
    score: u64,
    combo: String,
    prev_hits: Option<String>,
    hits: String,
    removed_misses: Option<u32>,
    map_info: String,
    mapset_id: u32,
}

impl SimulateEmbed {
    pub async fn new(
        score: Option<Score>,
        map: &Beatmap,
        mapset: &BeatmapsetCompact,
        args: SimulateArgs,
        ctx: &Context,
    ) -> BotResult<Self> {
        let is_some = args.is_some();

        let title = if map.mode == GameMode::Mania {
            format!(
                "{} {} - {} [{}]",
                osu::get_keys(GameMods::default(), map),
                mapset.artist.cow_escape_markdown(),
                mapset.title.cow_escape_markdown(),
                map.version.cow_escape_markdown(),
            )
        } else {
            format!(
                "{} - {} [{}]",
                mapset.artist.cow_escape_markdown(),
                mapset.title.cow_escape_markdown(),
                map.version.cow_escape_markdown(),
            )
        };

        let (prev_pp, prev_combo, prev_hits, misses) = if let Some(ref s) = score {
            let pp = if let Some(pp) = s.pp {
                pp
            } else {
                PpCalculator::new(ctx, map.map_id).await?.score(s).pp() as f32
            };

            let prev_combo = (map.mode == GameMode::Osu).then(|| s.max_combo);
            let prev_hits = Some(s.hits_string(map.mode));

            (
                Some(round(pp)),
                prev_combo,
                prev_hits,
                Some(s.statistics.count_miss),
            )
        } else {
            (None, None, None, None)
        };

        let mut unchoked_score = score.unwrap_or_else(default_score);
        let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
        let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;

        if let Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) = args.mods {
            unchoked_score.mods = mods;
        }

        let performance_attributes = rosu_map.max_pp(unchoked_score.mods.bits());
        let attributes = performance_attributes.difficulty_attributes();
        let max_pp = performance_attributes.pp() as f32;

        let stars = round(attributes.stars() as f32);

        if is_some {
            simulate_score(&mut unchoked_score, map, args, &attributes);
        } else {
            unchoke_score(&mut unchoked_score, map, &attributes);
        }

        let pp = rosu_map
            .pp()
            .attributes(attributes)
            .mods(unchoked_score.mods.bits())
            .combo(unchoked_score.max_combo as usize)
            .n300(unchoked_score.statistics.count_300 as usize)
            .n100(unchoked_score.statistics.count_100 as usize)
            .n50(unchoked_score.statistics.count_50 as usize)
            .n_katu(unchoked_score.statistics.count_katu as usize)
            .misses(unchoked_score.statistics.count_miss as usize)
            .score(unchoked_score.score)
            .calculate()
            .pp() as f32;

        let grade_completion_mods = grade_completion_mods(&unchoked_score, map);
        let pp = osu::get_pp(Some(pp), Some(max_pp));
        let hits = unchoked_score.hits_string(map.mode);

        let (combo, acc) = match map.mode {
            GameMode::Osu | GameMode::Catch => (
                osu::get_combo(&unchoked_score, map),
                round(unchoked_score.accuracy),
            ),
            GameMode::Mania => (String::from("**-**/-"), 100.0),
            GameMode::Taiko => {
                let acc = round(unchoked_score.accuracy);

                let combo = if is_some {
                    if unchoked_score.max_combo == 0 {
                        "**-**/-".to_owned()
                    } else {
                        format!("**{}x**/-", unchoked_score.max_combo)
                    }
                } else if let Some(combo) = map.max_combo {
                    format!("**{combo}x**/{combo}")
                } else {
                    "**-**/-".to_string()
                };

                (combo, acc)
            }
        };

        let footer = FooterBuilder::new(format!("{:?} map by {}", map.status, mapset.creator_name))
            .icon_url(format!("{AVATAR_URL}{}", mapset.creator_id));

        Ok(Self {
            title,
            url: map.url.to_owned(),
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id), // mapset.covers is empty :(
            grade_completion_mods,
            stars,
            score: unchoked_score.score as u64,
            mode: map.mode,
            acc,
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(map, unchoked_score.mods, stars),
            removed_misses: misses,
            prev_hits,
            prev_combo,
            prev_pp,
            mapset_id: mapset.mapset_id,
        })
    }

    pub fn as_maximized(&self) -> Embed {
        let image = format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            self.mapset_id
        );

        let combo = if let Some(prev_combo) = self.prev_combo {
            format!("{prev_combo} → {}", self.combo)
        } else {
            self.combo.to_owned()
        };

        let mut fields = vec![
            field!(
                "Grade",
                self.grade_completion_mods.as_ref().to_owned(),
                true
            ),
            field!("Acc", format!("{}%", self.acc), true),
            field!("Combo", combo, true),
        ];

        let pp = if let Some(prev_pp) = self.prev_pp {
            format!("{prev_pp} → {}", self.pp)
        } else {
            self.pp.to_owned()
        };

        fields.push(field!("PP", pp, true));

        if self.mode == GameMode::Mania {
            fields.push(field!(
                "Score",
                with_comma_int(self.score).to_string(),
                true
            ));
        } else {
            let hits = if let Some(ref prev_hits) = self.prev_hits {
                format!("{prev_hits} → {}", &self.hits)
            } else {
                self.hits.to_owned()
            };

            fields.push(field!("Hits", hits, true));
        }

        fields.push(field!("Map Info", self.map_info.clone(), false));

        EmbedBuilder::new()
            .fields(fields)
            .footer(&self.footer)
            .image(image)
            .title(&self.title)
            .url(&self.url)
            .build()
    }

    pub fn into_minimized(self) -> Embed {
        let mut value = if let Some(prev_pp) = self.prev_pp {
            format!("{prev_pp} → {}", self.pp)
        } else {
            self.pp
        };

        if self.mode != GameMode::Mania {
            let _ = write!(value, " {}", self.hits);
        }

        if let Some(misses) = self.removed_misses.filter(|misses| *misses > 0) {
            let _ = write!(value, " (+{misses}miss)");
        }

        let mut name = String::with_capacity(50);
        let _ = write!(name, "{} ", self.grade_completion_mods);

        if self.mode == GameMode::Mania {
            let _ = write!(name, "{} ", with_comma_int(self.score));
            let _ = write!(name, "({}%)", self.acc);
        } else {
            let _ = write!(name, "({}%)", self.acc);

            let _ = if let Some(prev_combo) = self.prev_combo {
                write!(name, " [ {prev_combo} → {} ]", self.combo)
            } else {
                write!(name, " [ {} ]", self.combo)
            };
        }

        let mut title = self.title;
        let _ = write!(title, " [{}★]", self.stars);

        EmbedBuilder::new()
            .fields(vec![field!(name, value, false)])
            .thumbnail(self.thumbnail)
            .title(title)
            .url(self.url)
            .build()
    }
}

fn simulate_score(
    score: &mut Score,
    map: &Beatmap,
    args: SimulateArgs,
    attributes: &DifficultyAttributes,
) {
    match attributes {
        DifficultyAttributes::Osu(attrs) => {
            let acc = args.acc.map_or(1.0, |a| a / 100.0);
            let mut n50 = args.n50.unwrap_or(0);
            let mut n100 = args.n100.unwrap_or(0);
            let mut miss = args.misses.unwrap_or(0);
            let n_objects = map.count_objects() as usize;

            let combo = args
                .combo
                .unwrap_or(attrs.max_combo)
                .min((attrs.max_combo).saturating_sub(miss));

            if n50 > 0 || n100 > 0 {
                let placed_points = 2 * n100 + n50 + miss;
                let missing_objects = n_objects - n100 - n50 - miss;
                let missing_points =
                    ((6.0 * acc * n_objects as f32).round() as usize).saturating_sub(placed_points);

                let mut n300 = missing_objects.min(missing_points / 6);
                n50 += missing_objects - n300;

                if let Some(orig_n50) = args.n50.filter(|_| args.n100.is_none()) {
                    // Only n50s were changed, try to load some off again onto n100s
                    let difference = n50 - orig_n50;
                    let n = n300.min(difference / 4);

                    n300 -= n;
                    n100 += 5 * n;
                    n50 -= 4 * n;
                }

                score.statistics.count_300 = n300 as u32;
                score.statistics.count_100 = n100 as u32;
                score.statistics.count_50 = n50 as u32;
            } else if miss > 0 && args.acc.is_none() {
                score.statistics.count_300 = n_objects.saturating_sub(miss) as u32;
                score.statistics.count_100 = 0;
                score.statistics.count_50 = 0;
            } else {
                let target_total = (acc * n_objects as f32 * 6.0).round() as usize;
                let left = n_objects.saturating_sub(miss);
                let delta = target_total.saturating_sub(left);

                // Increase miss count if acc cannot be fulfilled
                miss += left.saturating_sub(target_total);

                let mut n300 = delta / 5;
                let mut n100 = (delta % 5).min(n_objects - n300 - miss);
                let mut n50 = n_objects - n300 - n100 - miss;

                // Sacrifice n300s to transform n50s into n100s
                let n = n300.min(n50 / 4);
                n300 -= n;
                n100 += 5 * n;
                n50 -= 4 * n;

                score.statistics.count_300 = n300 as u32;
                score.statistics.count_100 = n100 as u32;
                score.statistics.count_50 = n50 as u32;
            }

            score.mode = GameMode::Osu;
            score.statistics.count_miss = miss as u32;
            score.max_combo = combo as u32;
            score.accuracy = score.accuracy();
            score.grade = score.grade(None);
        }
        DifficultyAttributes::Mania(_) => {
            let mut max_score = 1_000_000;

            let mods = score.mods;

            if mods.contains(GameMods::Easy) {
                max_score /= 2;
            }

            if mods.contains(GameMods::NoFail) {
                max_score /= 2;
            }

            if mods.contains(GameMods::HalfTime) {
                max_score /= 2;
            }

            score.mode = GameMode::Mania;
            score.max_combo = 0;
            score.score = args.score.map_or(max_score, |s| s.min(max_score));
            score.statistics.count_geki = map.count_objects();
            score.statistics.count_300 = 0;
            score.statistics.count_katu = 0;
            score.statistics.count_100 = 0;
            score.statistics.count_50 = 0;
            score.statistics.count_miss = 0;
            score.accuracy = 100.0;

            score.grade = if mods.intersects(GameMods::Flashlight | GameMods::Hidden) {
                if score.score == max_score {
                    Grade::XH
                } else {
                    Grade::SH
                }
            } else if score.score == max_score {
                Grade::X
            } else {
                Grade::S
            };
        }
        DifficultyAttributes::Taiko(_) => {
            let n100 = args.n100.unwrap_or(0);
            let n300 = args.n300.unwrap_or(0);
            let miss = args.misses.unwrap_or(0);
            let n_objects = map.count_circles as usize;
            let missing = n_objects - (n300 + n100 + miss);

            match args.acc {
                Some(acc) => {
                    let target_total = (acc * n_objects as f32 * 2.0 / 100.0).round() as usize;

                    let new300 = target_total
                        .saturating_sub(2 * n300)
                        .saturating_sub(n100)
                        .saturating_sub(missing);

                    let new100 = missing - new300;

                    score.statistics.count_300 = (n300 + new300) as u32;
                    score.statistics.count_100 = (n100 + new100) as u32;
                }
                None => {
                    score.statistics.count_300 = (n300 + missing) as u32;
                    score.statistics.count_100 = (n100) as u32;
                }
            }

            score.statistics.count_miss = miss as u32;

            let acc = (2 * score.statistics.count_300 + score.statistics.count_100) as f32
                / (2 * (score.statistics.count_300
                    + score.statistics.count_100
                    + score.statistics.count_miss)) as f32;

            score.max_combo = args
                .combo
                .map(|c| c as u32)
                .or(map.max_combo)
                .unwrap_or(0)
                .min(n_objects.saturating_sub(miss) as u32);

            score.mode = GameMode::Taiko;
            score.accuracy = acc * 100.0;
            score.grade = score.grade(Some(score.accuracy));
        }
        DifficultyAttributes::Catch(attrs) => {
            let n_tiny_droplets;
            let n_tiny_droplet_misses;
            let mut n_droplets;
            let mut n_fruits;

            let miss = attrs.max_combo().min(args.misses.unwrap_or(0));

            match args.acc {
                Some(acc) => {
                    n_droplets = match args.n100 {
                        Some(n100) => attrs.n_droplets.min(n100),
                        None => attrs.n_droplets.saturating_sub(miss),
                    };

                    n_fruits = attrs.n_fruits.saturating_sub(
                        miss.saturating_sub(attrs.n_droplets.saturating_sub(n_droplets)),
                    );

                    n_tiny_droplets = match args.n50 {
                        Some(n50) => attrs.n_tiny_droplets.min(n50),
                        None => ((acc / 100.0 * (attrs.max_combo() + attrs.n_tiny_droplets) as f32)
                            .round() as usize)
                            .saturating_sub(n_fruits)
                            .saturating_sub(n_droplets),
                    };

                    n_tiny_droplet_misses = attrs.n_tiny_droplets.saturating_sub(n_tiny_droplets);
                }
                None => {
                    n_droplets = attrs.n_droplets.min(args.n100.unwrap_or(0) as usize);
                    n_fruits = attrs.n_fruits.min(args.n300.unwrap_or(0) as usize);

                    let missing_fruits = attrs.n_fruits.saturating_sub(n_fruits);
                    let missing_droplets = attrs.n_droplets.saturating_sub(n_droplets);

                    n_droplets += missing_droplets.saturating_sub(miss);
                    n_fruits +=
                        missing_fruits.saturating_sub(miss.saturating_sub(missing_droplets));

                    n_tiny_droplets = match args.n50 {
                        Some(n50) => attrs.n_tiny_droplets.min(n50 as usize),
                        None => attrs.n_tiny_droplets,
                    };

                    n_tiny_droplet_misses = attrs.n_tiny_droplets - n_tiny_droplets;
                }
            }

            score.mode = GameMode::Catch;
            score.statistics.count_300 = n_fruits as u32;
            score.statistics.count_100 = n_droplets as u32;
            score.statistics.count_50 = n_tiny_droplets as u32;
            score.statistics.count_katu = n_tiny_droplet_misses as u32;
            score.statistics.count_miss = miss as u32;

            score.max_combo = args
                .combo
                .unwrap_or_else(|| attrs.max_combo())
                .min(attrs.max_combo() - miss) as u32;

            score.accuracy = score.accuracy();
            score.grade = score.grade(Some(score.accuracy));
        }
    }
}

fn unchoke_score(score: &mut Score, map: &Beatmap, attributes: &DifficultyAttributes) {
    match attributes {
        DifficultyAttributes::Osu(attrs)
            if score.statistics.count_miss > 0
                || score.max_combo < map.max_combo.unwrap_or(attrs.max_combo as u32) - 5 =>
        {
            let total_objects = map.count_objects() as usize;
            let passed_objects = score.total_hits() as usize;

            let mut count300 =
                score.statistics.count_300 as usize + total_objects.saturating_sub(passed_objects);

            let count_hits = total_objects - score.statistics.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

            count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.statistics.count_100 + new100s) as usize;

            score.statistics.count_300 = count300 as u32;
            score.statistics.count_100 = count100 as u32;
            score.max_combo = map.max_combo.unwrap_or(attrs.max_combo as u32);
        }
        DifficultyAttributes::Mania(_) => {
            score.score = 1_000_000;
            let mods = score.mods;

            if mods.contains(GameMods::Easy) {
                score.score /= 2;
            }

            if mods.contains(GameMods::NoFail) {
                score.score /= 2;
            }

            if mods.contains(GameMods::HalfTime) {
                score.score /= 2;
            }

            let hdfl = GameMods::Flashlight | GameMods::Hidden;

            score.grade = if score.mods.intersects(hdfl) {
                Grade::XH
            } else {
                Grade::X
            };

            return;
        }
        DifficultyAttributes::Catch(attrs)
            if score.max_combo != map.max_combo.unwrap_or_else(|| attrs.max_combo() as u32) =>
        {
            let total_objects = attrs.max_combo();
            let passed_objects = score.total_hits() as usize;

            let missing = total_objects - passed_objects;
            let missing_fruits = missing.saturating_sub(
                attrs
                    .n_droplets
                    .saturating_sub(score.statistics.count_100 as usize),
            );
            let missing_droplets = missing - missing_fruits;

            let n_fruits = score.statistics.count_300 as usize + missing_fruits;
            let n_droplets = score.statistics.count_100 as usize + missing_droplets;
            let n_tiny_droplet_misses = score.statistics.count_katu as usize;
            let n_tiny_droplets = attrs.n_tiny_droplets.saturating_sub(n_tiny_droplet_misses);

            score.statistics.count_300 = n_fruits as u32;
            score.statistics.count_100 = n_droplets as u32;
            score.statistics.count_katu = n_tiny_droplet_misses as u32;
            score.statistics.count_50 = n_tiny_droplets as u32;
            score.max_combo = total_objects as u32;
        }
        DifficultyAttributes::Taiko(_)
            if score.grade == Grade::F || score.statistics.count_miss > 0 =>
        {
            let total_objects = map.count_circles as usize;
            let passed_objects = score.total_hits() as usize;

            let mut count300 =
                score.statistics.count_300 as usize + total_objects.saturating_sub(passed_objects);

            let count_hits = total_objects - score.statistics.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

            count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.statistics.count_100 + new100s) as usize;

            score.statistics.count_300 = count300 as u32;
            score.statistics.count_100 = count100 as u32;
        }
        _ => return,
    }

    score.statistics.count_miss = 0;
    score.grade = score.grade(None);
    score.accuracy = score.accuracy();
}

fn default_score() -> Score {
    Score {
        accuracy: 100.0,
        ended_at: OffsetDateTime::now_utc(),
        grade: Grade::D,
        max_combo: 0,
        map: None,
        mapset: None,
        mode: GameMode::default(),
        mods: GameMods::default(),
        perfect: false,
        pp: None,
        rank_country: None,
        rank_global: None,
        replay: None,
        score: 0,
        score_id: 0,
        statistics: ScoreStatistics {
            count_geki: 0,
            count_300: 0,
            count_katu: 0,
            count_100: 0,
            count_50: 0,
            count_miss: 0,
        },
        user: None,
        user_id: 0,
        weight: None,
    }
}

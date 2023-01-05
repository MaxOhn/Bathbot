use std::{borrow::Cow, fmt::Write};

use bathbot_model::ScoreSlim;
use bathbot_util::{
    constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
    numbers::{round, WithComma},
    osu::{calculate_grade, ModSelection},
    CowUtils, EmbedBuilder, FooterBuilder,
};
use eyre::Result;
use rosu_pp::DifficultyAttributes;
use rosu_v2::prelude::{GameMode, GameMods, Grade, ScoreStatistics};
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{
        HasMods, ModsResult, RecentSimulateCatch, RecentSimulateMania, RecentSimulateOsu,
        RecentSimulateTaiko, SimulateEntry,
    },
    core::Context,
    embeds::osu,
    manager::OsuMap,
    util::osu::grade_completion_mods,
};

use super::{ComboFormatter, HitResultFormatter, KeyFormatter, PpFormatter};

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
    fn any_args(&self) -> bool {
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

    stars: f32,
    mode: GameMode,
    grade_completion_mods: Cow<'static, str>,
    acc: f32,
    prev_pp: Option<f32>,
    pp: PpFormatter,
    prev_combo: Option<u32>,
    score: u64,
    combo: String,
    prev_hits: Option<HitResultFormatter>,
    hits: HitResultFormatter,
    removed_misses: Option<u32>,
    map_info: String,
    mapset_id: u32,
}

impl SimulateEmbed {
    pub async fn new(entry: &SimulateEntry, args: SimulateArgs, ctx: &Context) -> Self {
        let SimulateEntry {
            original_score,
            if_fc,
            map,
            stars,
            max_pp,
        } = entry;

        let mut title = format!(
            "{} - {} [{}]",
            map.artist().cow_escape_markdown(),
            map.title().cow_escape_markdown(),
            map.version().cow_escape_markdown(),
        );

        if map.mode() == GameMode::Mania {
            let _ = write!(title, " [{}]", KeyFormatter::new(GameMods::NoMod, map));
        }

        let (prev_pp, prev_combo, prev_hits, misses) = if let Some(score) = original_score {
            let prev_combo = (score.mode == GameMode::Osu).then_some(score.max_combo);
            let prev_hits = Some(HitResultFormatter::new(
                score.mode,
                score.statistics.clone(),
            ));

            (
                Some(round(score.pp)),
                prev_combo,
                prev_hits,
                Some(score.statistics.count_miss),
            )
        } else {
            (None, None, None, None)
        };

        let mut unchoked_score = original_score.as_ref().map_or(
            ScoreSlim {
                accuracy: 100.0,
                ended_at: OffsetDateTime::now_utc(),
                grade: Grade::F,
                max_combo: 0,
                mode: map.mode(),
                mods: GameMods::NoMod,
                pp: 0.0,
                score: 0,
                score_id: None,
                statistics: ScoreStatistics {
                    count_geki: 0,
                    count_300: 0,
                    count_katu: 0,
                    count_100: 0,
                    count_50: 0,
                    count_miss: 0,
                },
            },
            ScoreSlim::clone,
        );

        if let Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) = args.mods {
            unchoked_score.mods = mods;
        }

        if args.any_args() {
            simulate_score(ctx, &mut unchoked_score, map, args).await;
        } else if let Some(if_fc) = if_fc {
            unchoked_score.grade =
                calculate_grade(unchoked_score.mode, unchoked_score.mods, &if_fc.statistics);

            unchoked_score.accuracy = if_fc.accuracy();
            unchoked_score.pp = if_fc.pp;
            unchoked_score.statistics = if_fc.statistics.clone();
            unchoked_score.max_combo = map.max_combo().unwrap_or(0);
        } else {
            perfect_score(ctx, &mut unchoked_score, map, *max_pp).await;
        }

        let grade_completion_mods = grade_completion_mods(
            unchoked_score.mods,
            unchoked_score.grade,
            unchoked_score.total_hits(),
            map,
        );

        let mode = map.mode();
        let pp = PpFormatter::new(Some(unchoked_score.pp), Some(*max_pp));
        let hits = HitResultFormatter::new(mode, unchoked_score.statistics.clone());
        let acc = round(unchoked_score.accuracy);
        let combo = ComboFormatter::new(unchoked_score.max_combo, map.max_combo()).to_string();

        let footer = FooterBuilder::new(format!("{:?} map", map.status()))
            .icon_url(format!("{AVATAR_URL}{}", map.creator_id()));

        Self {
            title,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id()),
            grade_completion_mods,
            stars: *stars,
            score: unchoked_score.score as u64,
            mode: map.mode(),
            acc,
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(map, unchoked_score.mods, *stars),
            removed_misses: misses,
            prev_hits,
            prev_combo,
            prev_pp,
            mapset_id: map.mapset_id(),
        }
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

        let mut fields = fields![
            "Grade", self.grade_completion_mods.as_ref().to_owned(), true;
            "Acc", format!("{}%", self.acc), true;
            "Combo", combo, true;
        ];

        let pp = if let Some(prev_pp) = self.prev_pp {
            format!("{prev_pp} → {}", self.pp)
        } else {
            self.pp.to_string()
        };

        fields![fields { "PP", pp, true }];

        if self.mode == GameMode::Mania {
            fields![fields { "Score", WithComma::new(self.score).to_string(), true }];
        } else {
            let hits = if let Some(ref prev_hits) = self.prev_hits {
                format!("{prev_hits} → {}", &self.hits)
            } else {
                self.hits.to_string()
            };

            fields![fields { "Hits", hits, true }];
        }

        fields![fields { "Map Info", self.map_info.clone(), false }];

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
            self.pp.to_string()
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
            let _ = write!(name, "{} ", WithComma::new(self.score));
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
        let _ = write!(title, " [{:.2}★]", self.stars);

        EmbedBuilder::new()
            .fields(fields![name, value, false])
            .thumbnail(self.thumbnail)
            .title(title)
            .url(self.url)
            .build()
    }
}

async fn simulate_score(ctx: &Context, score: &mut ScoreSlim, map: &OsuMap, args: SimulateArgs) {
    let mut calc = ctx.pp(map).mods(score.mods).mode(score.mode);
    let attrs = calc.difficulty().await;

    match attrs {
        DifficultyAttributes::Osu(attrs) => {
            let acc = args.acc.map_or(1.0, |a| a / 100.0);
            let mut n50 = args.n50.unwrap_or(0);
            let mut n100 = args.n100.unwrap_or(0);
            let mut miss = args.misses.unwrap_or(0);
            let n_objects = map.n_objects();

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
            score.accuracy = score.statistics.accuracy(GameMode::Osu);
        }
        DifficultyAttributes::Taiko(_) => {
            let n100 = args.n100.unwrap_or(0);
            let n300 = args.n300.unwrap_or(0);
            let miss = args.misses.unwrap_or(0);
            let n_objects = map.n_circles();
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
                .or_else(|| map.max_combo())
                .unwrap_or(0)
                .min(n_objects.saturating_sub(miss) as u32);

            score.mode = GameMode::Taiko;
            score.accuracy = acc * 100.0;
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
                    n_droplets = attrs.n_droplets.min(args.n100.unwrap_or(0));
                    n_fruits = attrs.n_fruits.min(args.n300.unwrap_or(0));

                    let missing_fruits = attrs.n_fruits.saturating_sub(n_fruits);
                    let missing_droplets = attrs.n_droplets.saturating_sub(n_droplets);

                    n_droplets += missing_droplets.saturating_sub(miss);
                    n_fruits +=
                        missing_fruits.saturating_sub(miss.saturating_sub(missing_droplets));

                    n_tiny_droplets = match args.n50 {
                        Some(n50) => attrs.n_tiny_droplets.min(n50),
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

            score.accuracy = score.statistics.accuracy(GameMode::Osu);
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
            score.statistics.count_geki = map.n_objects() as u32;
            score.statistics.count_300 = 0;
            score.statistics.count_katu = 0;
            score.statistics.count_100 = 0;
            score.statistics.count_50 = 0;
            score.statistics.count_miss = 0;
            score.accuracy = 100.0;
        }
    }

    score.grade = calculate_grade(score.mode, score.mods, &score.statistics);
    let pp = calc.score(&*score).performance().await.pp();
    score.pp = pp as f32;
}

async fn perfect_score(ctx: &Context, score: &mut ScoreSlim, map: &OsuMap, max_pp: f32) {
    let calc = ctx.pp(map);

    match calc.mods(score.mods).mode(score.mode).difficulty().await {
        DifficultyAttributes::Osu(attrs) => {
            score.statistics.count_300 = map.n_objects() as u32;
            score.max_combo = attrs.max_combo() as u32;
            score.pp = max_pp;
            score.grade = calculate_grade(GameMode::Osu, score.mods, &score.statistics);
        }
        DifficultyAttributes::Taiko(attrs) => {
            score.statistics.count_300 = map.n_circles() as u32;
            score.max_combo = attrs.max_combo() as u32;
            score.pp = max_pp;
            score.grade = calculate_grade(GameMode::Taiko, score.mods, &score.statistics);
        }
        DifficultyAttributes::Catch(attrs) => {
            score.statistics.count_300 = attrs.max_combo() as u32;
            score.max_combo = attrs.max_combo() as u32;
            score.pp = max_pp;
            score.grade = calculate_grade(GameMode::Catch, score.mods, &score.statistics);
        }
        DifficultyAttributes::Mania(attrs) => {
            score.statistics.count_geki = map.n_objects() as u32;
            score.max_combo = attrs.max_combo() as u32;
            score.pp = max_pp;
            score.grade = calculate_grade(GameMode::Mania, score.mods, &score.statistics);
        }
    }
}

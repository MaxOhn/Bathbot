use chrono::Utc;
use rosu_pp::{Beatmap as Map, BeatmapExt, DifficultyAttributes};
use rosu_v2::prelude::{Beatmap, GameMode, GameMods, Grade, Score, ScoreStatistics, User};

use crate::{
    commands::osu::OverrideArgs,
    core::Context,
    embeds::Author,
    error::PpError,
    util::{constants::MAP_THUMB_URL, osu::prepare_beatmap_file},
    BotResult,
};

pub struct OverrideEmbed {
    author: Author,
    description: String,
    thumbnail: String,
    title: String,
    url: String,
}

impl OverrideEmbed {
    pub async fn new(
        ctx: &Context,
        user: &User,
        map: Beatmap,
        score: Option<&Score>,
        best: &[Score],
        args: OverrideArgs,
    ) -> BotResult<Self> {
        let mapset = map.mapset.as_ref().unwrap();
        let title = format!("{} - {} [{}]", mapset.artist, mapset.title, map.version);

        let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
        let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;

        let mods = args
            .mods
            .map(|m| m.mods())
            .or_else(|| score.map(|s| s.mods))
            .unwrap_or_default()
            .bits();

        let difficulty = rosu_map.stars(mods, None);
        let stars = difficulty.stars();

        let mut simulated = score.cloned().unwrap_or_else(default_score);

        if score.is_some() && args.is_some() {
            modify_score(&mut simulated, &map, args, difficulty);
        } else {
            simulate(&mut simulated, &map, args, difficulty);
        }

        let description = match score {
            Some(score) => Self::with_score(user, &map, score, best),
            None => Self::without_score(user, &map, &simulated, best),
        };

        let author = author!(user);
        let url = map.url;
        let thumbnail = format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id);

        Ok(Self {
            author,
            description,
            thumbnail,
            title,
            url,
        })
    }

    fn without_score(user: &User, map: &Beatmap, simulated: &Score, best: &[Score]) -> String {
        todo!()
    }

    fn with_score(user: &User, map: &Beatmap, score: &Score, best: &[Score]) -> String {
        todo!()
    }
}

impl_builder!(OverrideEmbed {
    author,
    description,
    thumbnail,
    title,
    url,
});

fn simulate(
    score: &mut Score,
    map: &Beatmap,
    args: OverrideArgs,
    difficulty: DifficultyAttributes,
) {
    match difficulty {
        DifficultyAttributes::Fruits(ref attrs) => {
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

            score.mode = GameMode::CTB;
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

            // attributes = DifficultyAttributes::Fruits(attrs);
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

            score.mode = GameMode::MNA;
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

            // attributes = DifficultyAttributes::Mania(attrs);
        }
        DifficultyAttributes::Osu(ref attrs) => {
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

            score.mode = GameMode::STD;
            score.statistics.count_miss = miss as u32;
            score.max_combo = combo as u32;
            score.accuracy = score.accuracy();
            score.grade = score.grade(None);

            // attributes = DifficultyAttributes::Osu(attrs);
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

            score.mode = GameMode::TKO;
            score.accuracy = acc * 100.0;
            score.grade = score.grade(Some(score.accuracy));

            // attributes = DifficultyAttributes::Taiko(attrs);
        }
    }
}

fn modify_score(
    score: &mut Score,
    map: &Beatmap,
    args: OverrideArgs,
    difficulty: DifficultyAttributes,
) {
    let OverrideArgs {
        mods,
        n300,
        n100,
        n50,
        misses,
        acc,
        combo,
        score: score_,
        ..
    } = args;

    if let Some(mods) = mods {
        score.mods = mods.mods();
    }

    if let Some(n300) = n300 {
        score.statistics.count_300 = n300 as u32;
    }

    if let Some(n100) = n100 {
        score.statistics.count_100 = n100 as u32;
    }

    if let Some(n50) = n50 {
        score.statistics.count_50 = n50 as u32;
    }

    if let Some(misses) = misses {
        score.statistics.count_miss = misses as u32;
    }

    if let Some(combo) = combo {
        score.max_combo = combo as u32;
    }

    match difficulty {
        DifficultyAttributes::Osu(ref attrs) => {
            let n_objects = map.count_objects() as usize;

            if let Some(acc) = acc {
                let mut miss = args.misses.unwrap_or(0);
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
                score.statistics.count_miss = miss as u32;

                return;
            } else {
                let passed_objects = score.total_hits() as usize;
                let mut count300 =
                    score.statistics.count_300 as usize + n_objects.saturating_sub(passed_objects);

                let count_hits = n_objects - score.statistics.count_miss as usize;
                let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                let count100 = (score.statistics.count_100 + new100s) as usize;

                score.statistics.count_300 = count300 as u32;
                score.statistics.count_100 = count100 as u32;
                score.max_combo = map.max_combo.unwrap_or(attrs.max_combo as u32);
            }
        }
        DifficultyAttributes::Mania(_) => {
            score.score = score_.unwrap_or_else(|| {
                let mods = score.mods;
                let mut score = 1_000_000;

                if mods.contains(GameMods::Easy) {
                    score /= 2;
                }

                if mods.contains(GameMods::NoFail) {
                    score /= 2;
                }

                if mods.contains(GameMods::HalfTime) {
                    score /= 2;
                }

                score
            });

            let hdfl = GameMods::Flashlight | GameMods::Hidden;

            score.grade = if score.mods.intersects(hdfl) {
                Grade::XH
            } else {
                Grade::X
            };

            return;
        }
        DifficultyAttributes::Fruits(ref attrs) => {
            let total_objects = attrs.max_combo();
            let passed_objects = score.total_hits() as usize;

            // TODO: consider acc

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
        DifficultyAttributes::Taiko(_) => {
            let total_objects = map.count_circles as usize;
            let passed_objects = score.total_hits() as usize;

            // TODO: consider acc

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
    }

    score.statistics.count_miss = 0;
    score.grade = score.grade(None);
    score.accuracy = score.accuracy();
}

fn default_score() -> Score {
    Score {
        accuracy: 100.0,
        created_at: Utc::now(),
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
        replay: false,
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

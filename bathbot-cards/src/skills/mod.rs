mod description;
mod prefix;
mod suffix;
mod title;

use std::hash::BuildHasher;

use rosu_pp::{
    catch::{CatchPerformanceAttributes, CatchScoreState},
    mania::ManiaScoreState,
    osu::{OsuPP, OsuScoreState},
    taiko::TaikoScoreState,
    AnyPP, BeatmapExt, GameMode as Mode,
};
use rosu_v2::model::{score::Score, GameMode};

pub(crate) use self::{
    description::TitleDescriptions, prefix::TitlePrefix, suffix::TitleSuffix, title::CardTitle,
};
use crate::card::Maps;

pub enum Skills {
    Osu { acc: f64, aim: f64, speed: f64 },
    Taiko { acc: f64, strain: f64 },
    Catch { acc: f64, movement: f64 },
    Mania { acc: f64, strain: f64 },
}

impl Skills {
    pub fn calculate<S>(mode: GameMode, scores: &[Score], mut maps: Maps<S>) -> Self
    where
        S: BuildHasher,
    {
        // https://www.desmos.com/calculator/gqnhbpa0d3
        let map = |val: f64| {
            let factor = (8.0 / (val / 72.0 + 8.0)).powi(10);

            -101.0 * factor + 101.0
        };

        match mode {
            GameMode::Osu => {
                let mut acc = 0.0;
                let mut aim = 0.0;
                let mut speed = 0.0;
                let mut weight_sum = 0.0;

                const ACC_NERF: f64 = 1.3;
                const AIM_NERF: f64 = 2.6;
                const SPEED_NERF: f64 = 2.4;

                for (i, score) in scores.iter().enumerate() {
                    let Some((map, attrs)) = maps.remove(&score.map_id) else {
                        continue;
                    };

                    let state = OsuScoreState {
                        max_combo: score.max_combo as usize,
                        n300: score.statistics.count_300 as usize,
                        n100: score.statistics.count_100 as usize,
                        n50: score.statistics.count_50 as usize,
                        n_misses: score.statistics.count_miss as usize,
                    };

                    let attrs = OsuPP::new(&map)
                        .attributes(attrs)
                        .mods(score.mods.bits())
                        .state(state)
                        .calculate();

                    let acc_val = attrs.pp_acc / ACC_NERF;
                    let aim_val = attrs.pp_aim / AIM_NERF;
                    let speed_val = attrs.pp_speed / SPEED_NERF;
                    let weight = 0.95_f64.powi(i as i32);

                    acc += acc_val * weight;
                    aim += aim_val * weight;
                    speed += speed_val * weight;
                    weight_sum += weight;
                }

                acc = map(acc / weight_sum);
                aim = map(aim / weight_sum);
                speed = map(speed / weight_sum);

                Self::Osu { acc, aim, speed }
            }
            GameMode::Taiko => {
                let mut acc = 0.0;
                let mut strain = 0.0;
                let mut weight_sum = 0.0;

                const ACC_NERF: f64 = 1.15;
                const DIFFICULTY_NERF: f64 = 2.8;

                for (i, score) in scores.iter().enumerate() {
                    let Some((map, attrs)) = maps.remove(&score.map_id) else {
                        continue;
                    };

                    let state = TaikoScoreState {
                        max_combo: score.max_combo as usize,
                        n300: score.statistics.count_300 as usize,
                        n100: score.statistics.count_100 as usize,
                        n_misses: score.statistics.count_miss as usize,
                    };

                    let AnyPP::Taiko(calc) = map.pp().mode(Mode::Taiko) else {
                        unreachable!()
                    };

                    let attrs = calc
                        .attributes(attrs)
                        .mods(score.mods.bits())
                        .state(state)
                        .calculate();

                    let acc_val = attrs.pp_acc / ACC_NERF;
                    let difficulty_val = attrs.pp_difficulty / DIFFICULTY_NERF;
                    let weight = 0.95_f64.powi(i as i32);

                    acc += acc_val * weight;
                    strain += difficulty_val * weight;
                    weight_sum += weight;
                }

                acc = map(acc / weight_sum);
                strain = map(strain / weight_sum);

                Self::Taiko { acc, strain }
            }
            GameMode::Catch => {
                let mut acc = 0.0;
                let mut movement = 0.0;
                let mut weight_sum = 0.0;

                const ACC_BUFF: f64 = 2.0;
                const MOVEMENT_NERF: f64 = 4.7;

                for (i, score) in scores.iter().enumerate() {
                    let Some((map, attrs)) = maps.remove(&score.map_id) else {
                        continue;
                    };

                    let state = CatchScoreState {
                        max_combo: score.max_combo as usize,
                        n_fruits: score.statistics.count_300 as usize,
                        n_droplets: score.statistics.count_100 as usize,
                        n_tiny_droplets: score.statistics.count_50 as usize,
                        n_tiny_droplet_misses: score.statistics.count_katu as usize,
                        n_misses: score.statistics.count_miss as usize,
                    };

                    let AnyPP::Catch(calc) = map.pp().mode(Mode::Catch) else {
                        unreachable!()
                    };

                    let attrs = calc
                        .attributes(attrs)
                        .mods(score.mods.bits())
                        .state(state)
                        .calculate();

                    let CatchPerformanceAttributes { difficulty, pp } = attrs;

                    let acc_ = score.accuracy as f64;
                    let od = map.od as f64;

                    let n_objects = (difficulty.n_fruits
                        + difficulty.n_droplets
                        + difficulty.n_tiny_droplets) as f64;

                    // https://www.desmos.com/calculator/cg59pywpry
                    let acc_exp = ((acc_ / 46.5).powi(6) / 55.0).powf(1.5);
                    let acc_adj = (5.0 * acc_exp.powf(0.1).ln_1p()).recip();

                    let acc_val = difficulty.stars.powf(acc_exp - acc_adj)
                        * (od / 7.0).powf(0.25)
                        * (n_objects / 2000.0).powf(0.15)
                        * ACC_BUFF;

                    let movement_val = pp / MOVEMENT_NERF;
                    let weight = 0.95_f64.powi(i as i32);

                    acc += acc_val * weight;
                    movement += movement_val * weight;
                    weight_sum += weight;
                }

                acc = map(acc / weight_sum);
                movement = map(movement / weight_sum);

                Self::Catch { acc, movement }
            }
            GameMode::Mania => {
                let mut acc = 0.0;
                let mut strain = 0.0;
                let mut weight_sum = 0.0;

                const ACC_BUFF: f64 = 2.1;
                const DIFFICULTY_NERF: f64 = 0.6;

                for (i, score) in scores.iter().enumerate() {
                    let Some((map, attrs)) = maps.remove(&score.map_id) else {
                        continue;
                    };

                    let state = ManiaScoreState {
                        n320: score.statistics.count_geki as usize,
                        n300: score.statistics.count_300 as usize,
                        n200: score.statistics.count_katu as usize,
                        n100: score.statistics.count_100 as usize,
                        n50: score.statistics.count_50 as usize,
                        n_misses: score.statistics.count_miss as usize,
                    };

                    let AnyPP::Mania(calc) = map.pp().mode(Mode::Mania) else {
                        unreachable!()
                    };

                    let attrs = calc
                        .attributes(attrs)
                        .mods(score.mods.bits())
                        .state(state)
                        .calculate();

                    let acc_ = score.accuracy as f64;
                    let od = score.map.as_ref().unwrap().od as f64;
                    let n_objects = score.total_hits() as f64;

                    // https://www.desmos.com/calculator/b30p1awwft
                    let acc_ = ((acc_ / 36.0).powf(4.5) / 60.0).powf(1.5);

                    let acc_val = attrs.stars().powf(acc_)
                        * (od / 7.0).powf(0.25)
                        * (n_objects / 2000.0).powf(0.15)
                        * ACC_BUFF;

                    let difficulty_val = attrs.pp_difficulty / DIFFICULTY_NERF;
                    let weight = 0.95_f64.powi(i as i32);

                    acc += acc_val * weight;
                    strain += difficulty_val * weight;
                    weight_sum += weight;
                }

                acc = map(acc / weight_sum);
                strain = map(strain / weight_sum);

                Self::Mania { acc, strain }
            }
        }
    }

    pub(crate) fn mode(&self) -> GameMode {
        match self {
            Skills::Osu { .. } => GameMode::Osu,
            Skills::Taiko { .. } => GameMode::Taiko,
            Skills::Catch { .. } => GameMode::Catch,
            Skills::Mania { .. } => GameMode::Mania,
        }
    }
}

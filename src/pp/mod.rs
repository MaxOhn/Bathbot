pub mod roppai;

use roppai::Oppai;

use crate::{
    util::{error::PPError, osu::prepare_beatmap_file, BeatmapExt, ScoreExt},
    BotResult, Context, CONFIG,
};

use bitflags::bitflags;
use rosu::model::{
    ApprovalStatus::{Approved, Loved, Ranked},
    GameMode, GameMods,
};
use std::{collections::HashMap, str::FromStr};
use tokio::{process::Command, time};

bitflags! {
    pub struct Calculations: u8 {
        const PP = 1;
        const MAX_PP = 2;
        const STARS = 4;
    }
}

type TripleOption = (Option<f32>, Option<f32>, Option<f32>);

pub struct PPCalculator<'s, 'm> {
    score: Option<Box<dyn ScoreExt + 's>>,
    map: Option<Box<dyn BeatmapExt + 'm>>,

    mods: Option<GameMods>,

    pp: Option<f32>,
    max_pp: Option<f32>,
    stars: Option<f32>,
}

impl<'s, 'm> PPCalculator<'s, 'm> {
    pub fn new() -> Self {
        Self {
            score: None,
            map: None,
            mods: None,
            pp: None,
            max_pp: None,
            stars: None,
        }
    }
    pub fn mods(mut self, mods: GameMods) -> Self {
        self.mods.replace(mods);
        self
    }
    pub fn score(mut self, score: impl ScoreExt + 's) -> Self {
        self.score.replace(Box::new(score));
        self
    }
    pub fn map(mut self, map: impl BeatmapExt + 'm) -> Self {
        self.map.replace(Box::new(map));
        self
    }
    pub async fn calculate(&mut self, calcs: Calculations, ctx: Option<&Context>) -> BotResult<()> {
        assert_ne!(calcs.bits, 0);
        let map = match self.map.as_deref() {
            Some(map) => map,
            None => return Err(PPError::NoMapId.into()),
        };
        let score = self.score.as_deref();
        let map_path = prepare_beatmap_file(map.map_id()).await?;
        let (pp, max_pp, stars) = match map.mode() {
            GameMode::STD | GameMode::TKO => {
                calculate_oppai(calcs, self.mods, score, map, map_path)?
            }
            mode => {
                let ctx = ctx.ok_or(PPError::NoContext(mode))?;
                let stars = self
                    .calculate_osu_tools_stars(ctx, map_path, mode)
                    .await
                    .map_err(Box::new)
                    .map_err(PPError::Stars)?
                    .unwrap_or_default();
                if mode == GameMode::MNA {
                    calculate_mania(calcs, stars, self.mods, score, map)
                } else {
                    calculate_ctb(calcs, stars, self.mods, score, map)
                }
            }
        };
        if let Some(pp) = pp {
            self.pp.replace(pp);
        }
        if let Some(pp) = max_pp {
            self.max_pp.replace(pp);
        }
        if let Some(stars) = stars {
            self.stars.replace(stars);
        }
        Ok(())
    }

    pub fn pp(&self) -> Option<f32> {
        self.pp
    }
    pub fn max_pp(&self) -> Option<f32> {
        self.max_pp
    }
    pub fn stars(&self) -> Option<f32> {
        self.stars
    }

    async fn calculate_osu_tools_stars(
        &self,
        ctx: &Context,
        map_path: String,
        mode: GameMode,
    ) -> Result<Option<f32>, PPError> {
        // Note: If self.map was None, preparing the map_path would have panicked already
        let map = self.map.as_ref().unwrap();
        let mods = self.score.as_ref().map(|score| score.mods());

        // If mods dont change stars, return default stars
        if let Some(mods) = mods {
            if !mods.changes_stars(mode) && map.stars().is_some() {
                return Ok(map.stars());
            }
        }

        // Is value already stored?
        let star_option = ctx.stars(mode).get(&map.map_id()).and_then(|mod_map| {
            mod_map
                .get(&mods.unwrap_or_default())
                .map(|(stars, _)| *stars)
        });
        if let Some(stars) = star_option {
            return Ok(Some(stars));
        }
        let mut cmd = Command::new("dotnet");
        cmd.kill_on_drop(true)
            .arg(CONFIG.get().unwrap().perf_calc_path.as_os_str())
            .arg("difficulty")
            .arg(map_path);
        if let Some(mods) = mods {
            if !mods.is_empty() {
                for m in mods.iter().filter(|&m| m != GameMods::ScoreV2) {
                    cmd.arg("-m").arg(m.to_string());
                }
            }
        }
        let stars = parse_calculation(cmd, ctx).await?;
        // Store value
        if let Ranked | Loved | Approved = map.approval_status() {
            ctx.stars(mode)
                .entry(map.map_id())
                .and_modify(|value_map| {
                    value_map.insert(mods.unwrap_or_default(), (stars, true));
                })
                .or_insert_with(|| {
                    let mut value_map = HashMap::new();
                    value_map.insert(mods.unwrap_or_default(), (stars, true));
                    value_map
                });
        }
        Ok(Some(stars))
    }
}

fn calculate_oppai(
    calcs: Calculations,
    mods: Option<GameMods>,
    score: Option<&dyn ScoreExt>,
    map: &dyn BeatmapExt,
    map_path: String,
) -> BotResult<TripleOption> {
    let mut pp = None;
    let mut max_pp = None;
    let mut stars = None;

    let mut oppai = Oppai::new();

    if let Some(mods) = mods {
        oppai.set_mods(mods.bits());
    } else if let Some(score) = score {
        oppai.set_mods(score.mods().bits());
    }

    let calculated = if calcs.contains(Calculations::MAX_PP) {
        oppai.calculate(map_path.as_str())?;
        max_pp = Some(oppai.get_pp());
        true
    } else {
        false
    };

    // Needs to be done before PP in case of fail i.e. partial pass
    if calcs.contains(Calculations::STARS) {
        if !calculated {
            oppai.calculate(map_path.as_str())?;
        }
        stars = Some(oppai.get_stars());
    }

    if calcs.contains(Calculations::PP) {
        let (misses, n100, n50, combo, hits) = match score {
            Some(score) => (
                score.count_miss(),
                score.count_100(),
                score.count_50(),
                score.max_combo(),
                score.hits(map.mode()),
            ),
            None => (
                0,
                0,
                0,
                map.max_combo().unwrap_or(0),
                map.n_objects().unwrap_or(0),
            ),
        };

        oppai.set_miss_count(misses);
        oppai.set_hits(n100, n50);
        oppai.set_combo(combo);
        oppai.set_end_index(hits);
        oppai.calculate(map_path.as_str())?;
        pp = Some(oppai.get_pp());
    }

    Ok((pp, max_pp, stars))
}

fn calculate_mania(
    calcs: Calculations,
    stars: f32,
    _mods: Option<GameMods>,
    score: Option<&dyn ScoreExt>,
    map: &dyn BeatmapExt,
) -> TripleOption {
    let (mut mods, score_points) = match score {
        Some(score) => (score.mods(), score.score()),
        None => (GameMods::NoMod, 1_000_000),
    };

    if let Some(m) = _mods {
        mods = m;
    }

    let ez = mods.contains(GameMods::Easy);
    let nf = mods.contains(GameMods::NoFail);
    let ht = mods.contains(GameMods::HalfTime);

    let nerf_od = if ez { 0.5 } else { 1.0 };
    let nerf_pp = nerf_od * if nf { 0.9 } else { 1.0 };
    let nerf_score = 0.5_f32.powi(ez as i32 + nf as i32 + ht as i32);

    let n_objects = map
        .n_objects()
        .or_else(|| score.map(|score| score.hits(GameMode::MNA)))
        .unwrap_or(0);
    let od = map.od() / nerf_od;

    let mut pp = None;
    let mut max_pp = None;

    if calcs.contains(Calculations::PP) {
        pp = Some(mania_pp(
            score_points as f32 / nerf_score,
            n_objects,
            stars,
            od,
            nerf_od,
            nerf_pp,
        ));
    }
    if calcs.contains(Calculations::MAX_PP) {
        max_pp = Some(mania_pp(
            1_000_000.0,
            n_objects,
            stars,
            od,
            nerf_od,
            nerf_pp,
        ));
    }

    (pp, max_pp, Some(stars))
}

// Formula from http://maniapp.uy.to/ (2020.11.26)
fn mania_pp(score: f32, n_objects: u32, stars: f32, od: f32, nerf_od: f32, nerf_pp: f32) -> f32 {
    let strain_multiplier = if score < 500_000.0 {
        return 0.0;
    } else if score < 600_000.0 {
        (score - 500_000.0) / 100_000.0 * 0.3
    } else if score < 700_000.0 {
        (score - 600_000.0) / 100_000.0 * 0.25 + 0.3
    } else if score < 800_000.0 {
        (score - 700_000.0) / 100_000.0 * 0.2 + 0.55
    } else if score < 900_000.0 {
        (score - 800_000.0) / 100_000.0 * 0.15 + 0.75
    } else {
        (score - 900_000.0) / 100_000.0 * 0.1 + 0.9
    };

    let strain_base = (5.0 * (stars * 5.0).max(1.0) - 4.0).powf(2.2) / 135.0
        * (1.0 + (n_objects as f32 / 15_000.0).min(0.1));

    let acc_value = if score >= 960_000.0 {
        od * nerf_od * 0.02 * strain_base * ((score - 960_000.0) / 40_000.0).powf(1.1)
    } else {
        0.0
    };

    0.73 * (acc_value.powf(1.1) + (strain_base * strain_multiplier).powf(1.1)).powf(1.0 / 1.1)
        * 1.1
        * nerf_pp
}

fn calculate_ctb(
    calcs: Calculations,
    stars: f32,
    _mods: Option<GameMods>,
    score: Option<&dyn ScoreExt>,
    map: &dyn BeatmapExt,
) -> TripleOption {
    let max_combo = map.max_combo().unwrap_or(0);
    let ar = map.ar();

    let (acc, combo, misses, mut mods) = match score {
        Some(score) => (
            score.acc(GameMode::CTB),
            score.max_combo(),
            score.count_miss(),
            score.mods(),
        ),
        None => (100.0, max_combo, 0, GameMods::NoMod),
    };

    if let Some(m) = _mods {
        mods = m;
    }

    let mut pp = None;
    let mut max_pp = None;

    if calcs.contains(Calculations::PP) {
        pp = Some(ctb_pp(stars, acc, combo, max_combo, misses, ar, mods));
    }
    if calcs.contains(Calculations::MAX_PP) {
        max_pp = Some(ctb_pp(stars, acc, max_combo, max_combo, 0, ar, mods));
    }

    (pp, max_pp, Some(stars))
}

// Formula from https://pakachan.github.io/osustuff/ppcalculator.html (2020.11.26)
fn ctb_pp(
    stars: f32,
    acc: f32,
    combo: u32,
    max_combo: u32,
    misses: u32,
    ar: f32,
    mods: GameMods,
) -> f32 {
    let mut pp = (5.0 * stars / 0.0049 - 4.0).powi(2) / 100_000.0;

    // Length bonus
    let mut length_bonus = 0.95 + 0.3 * (max_combo as f32 / 2500.0).min(1.0);
    if max_combo > 2500 {
        length_bonus += (max_combo as f32 / 2500.0).log10() * 0.475;
    }
    pp *= length_bonus;

    // Miss penalty
    pp *= 0.97_f32.powi(misses as i32);

    // No FC penalty
    pp *= (combo as f32 / max_combo as f32).powf(0.8);

    // AR bonus
    let mut ar_bonus = 1.0;
    if ar > 9.0 {
        ar_bonus += 0.1 * (ar - 9.0);
        if ar > 10.0 {
            ar_bonus += 0.1 * (ar - 10.0);
        }
    } else if ar < 8.0 {
        ar_bonus += 0.025 * (8.0 - ar);
    }
    pp *= ar_bonus;

    // Acc penalty
    pp *= (acc / 100.0).powf(5.5);

    // Hidden bonus
    if mods.contains(GameMods::Hidden) {
        let hidden_bonus = if ar > 10.0 {
            1.01 + 0.04 * (11.0 - ar.min(11.0))
        } else {
            1.05 + 0.075 * (10.0 - ar)
        };
        pp *= hidden_bonus;
    }

    // Flashlight bonus
    if mods.contains(GameMods::Flashlight) {
        pp *= 1.35 * length_bonus;
    }

    pp
}

// async fn calculate_pp(
//     data: &PPCalculator,
//     calculations: Calculations,
//     map_path: &str,
//     ctx: Option<&Context>,
// ) -> Result<Option<f32>, PPError> {
//     // Do we want to calculate pp?
//     if !calculations.contains(Calculations::PP) {
//         return Ok(None);
//     }
//     // Is pp value already present?
//     if data.pp.is_some() {
//         return Ok(data.pp);
//     }
//     let mode = data.mode.unwrap();
//     // Distinguish between mods
//     let pp = match mode {
//         // Oppai for STD and TKO
//         GameMode::STD | GameMode::TKO => {
//             let mut oppai = Oppai::new();
//             if let Some(mods) = data.mods {
//                 oppai.set_mods(mods.bits());
//             }
//             if let Some(count_miss) = data.count_miss {
//                 oppai.set_miss_count(count_miss);
//             }
//             if let Some(count_100) = data.count_100 {
//                 if let Some(count_50) = data.count_50 {
//                     oppai.set_hits(count_100, count_50);
//                 }
//             }
//             if let Some(max_combo) = data.max_combo_score {
//                 oppai.set_combo(max_combo);
//             }
//             if let Some(total_hits) = data.total_hits_oppai() {
//                 oppai.set_end_index(total_hits);
//             }
//             oppai.calculate(map_path)?.get_pp()
//         }
//         // osu-tools for MNA and CTB
//         GameMode::MNA | GameMode::CTB => {
//              // UNNECESSARY
//         }
//     };
//     Ok(Some(pp))
// }

// async fn calculate_max_pp(
//     data: &PPCalculator,
//     calculations: Calculations,
//     map_path: &str,
//     ctx: Option<&Context>,
// ) -> Result<Option<f32>, PPError> {
//     // Do we want to calculate max pp?
//     if !calculations.contains(Calculations::MAX_PP) {
//         return Ok(None);
//     }
//     let map_id = data.map_id.unwrap();
//     let mode = data.mode.unwrap();
//     // Distinguish between mods
//     match mode {
//         // Oppai for STD and TKO
//         GameMode::STD | GameMode::TKO => {
//             let mut oppai = Oppai::new();
//             if let Some(mods) = data.mods {
//                 oppai.set_mods(mods.bits());
//             }
//             Ok(Some(oppai.calculate(map_path)?.get_pp()))
//         }
//         // osu-tools for MNA and CTB
//         GameMode::MNA | GameMode::CTB => {
//              // UNNECESSARY
//         }
//     }
// }

// async fn calculate_stars(
//     data: &PPCalculator,
//     calculations: Calculations,
//     map_path: &str,
//     ctx: Option<&Context>,
// ) -> Result<Option<f32>, PPError> {
//     if !calculations.contains(Calculations::STARS) {
//         return Ok(None);
//     }
//     if let Some(mode) = data.mode {
//         if let Some(mods) = data.mods {
//             if !mods.changes_stars(mode) && data.default_stars.is_some() {
//                 return Ok(data.default_stars);
//             }
//         }
//     }
//     let mode = data.mode.unwrap();
//     let map_id = data.map_id.unwrap();
//     match mode {
//         GameMode::STD | GameMode::TKO => {
//             let mut oppai = Oppai::new();
//             if let Some(mods) = data.mods {
//                 oppai.set_mods(mods.bits());
//             }
//             Ok(Some(oppai.calculate(map_path)?.get_stars()))
//         }
//         GameMode::MNA | GameMode::CTB => {
//             // DONE
//         }
//     }
// }

async fn parse_calculation(mut cmd: Command, ctx: &Context) -> Result<f32, PPError> {
    let calculation = time::timeout(time::Duration::from_secs(10), cmd.output());
    let output = {
        let _lock = ctx.pp_lock().lock().await;
        match calculation.await {
            Ok(output) => output.map_err(PPError::IoError)?,
            Err(_) => return Err(PPError::Timeout),
        }
    };
    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).into_owned();
        f32::from_str(&result.trim()).map_err(|_| PPError::InvalidFloat(result))
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(PPError::CommandLine(err_msg))
    }
}

use crate::{
    database::MySQL, roppai::Oppai, scraper::ScraperScore, util::osu, Error,
    PerformanceCalculatorLock,
};

use rosu::models::{ApprovalStatus, Beatmap, GameMod, GameMode, GameMods, Grade, Score};
use serenity::prelude::Context;
use std::{
    env, mem,
    process::{Child, Command, Stdio},
    str::FromStr,
};

pub enum PPProvider {
    Oppai {
        oppai: Oppai,
        pp: f32,
        max_pp: f32,
        stars: f32,
    },
    Mania {
        pp: f32,
        max_pp: f32,
        stars: f32,
    },
    Fruits {
        stars: f32,
    },
}

impl PPProvider {
    /// ctx is only required for mania
    pub fn new(score: &Score, map: &Beatmap, ctx: Option<&Context>) -> Result<Self, Error> {
        match map.mode {
            GameMode::STD | GameMode::TKO => {
                let mut oppai = Oppai::new();
                if !score.enabled_mods.is_empty() {
                    let bits = score.enabled_mods.as_bits();
                    oppai.set_mods(bits);
                }
                let map_path = osu::prepare_beatmap_file(map.beatmap_id)?;
                let max_pp = oppai.calculate(Some(&map_path))?.get_pp();
                oppai
                    .set_miss_count(score.count_miss)
                    .set_hits(score.count100, score.count50)
                    .set_end_index(score.total_hits(map.mode))
                    .set_combo(score.max_combo)
                    .calculate(None)?;
                let pp = oppai.get_pp();
                let stars = if score.enabled_mods.changes_stars(map.mode) {
                    oppai.get_stars()
                } else {
                    map.stars
                };
                Ok(Self::Oppai {
                    oppai,
                    pp,
                    max_pp,
                    stars,
                })
            }
            GameMode::MNA => {
                let ctx = ctx.unwrap();
                let mutex = if score.pp.is_none() {
                    let data = ctx.data.read();
                    Some(
                        data.get::<PerformanceCalculatorLock>()
                            .expect("Could not get PerformanceCalculatorLock")
                            .clone(),
                    )
                } else {
                    None
                };
                let half_score = half_score(&score.enabled_mods);
                let (stars, stars_in_db) = if score.enabled_mods.changes_stars(GameMode::MNA) {
                    // Try retrieving stars from database
                    let data = ctx.data.read();
                    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                    match mysql.get_mania_mod_stars(map.beatmap_id, &score.enabled_mods) {
                        Ok(result) => (result, true),
                        Err(why) => {
                            if let Error::Custom(_) = why {
                                warn!("Error while retrieving from stars_mania_mods: {}", why);
                            }
                            (None, false)
                        }
                    }
                } else {
                    (Some(map.stars), true)
                };
                // Start calculating pp of the score in new thread
                let (pp_child, lock) = if score.pp.is_none() {
                    // If its a fail or below half score, it's gonna be 0pp anyway
                    if score.grade == Grade::F || score.score < half_score as u32 {
                        (None, None)
                    } else {
                        let lock = mutex.as_ref().unwrap().lock();
                        let child =
                            start_pp_calc(map.beatmap_id, &score.enabled_mods, Some(score.score))?;
                        (Some(child), Some(lock))
                    }
                } else {
                    (None, None)
                };
                // Try retrieving max pp of the map from database
                let (max_pp, map_in_db) = {
                    let data = ctx.data.read();
                    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                    match mysql.get_mania_mod_pp(map.beatmap_id, &score.enabled_mods) {
                        Ok(result) => (result, true),
                        Err(why) => {
                            if let Error::Custom(_) = why {
                                warn!(
                                    "Some mod bit error for mods {} in pp_mania_mods table",
                                    score.enabled_mods
                                );
                            }
                            (None, false)
                        }
                    }
                };
                // Wait for score pp calculation to finish
                let pp = if let Some(pp_child) = pp_child {
                    parse_pp_calc(pp_child)?
                } else if score.grade == Grade::F || score.score < half_score as u32 {
                    0.0
                } else {
                    score.pp.unwrap()
                };
                // If max pp were found, get them
                let max_pp = if let Some(max_pp) = max_pp {
                    max_pp
                // Otherwise start calculating them in new thread
                } else {
                    let max_pp_child = start_pp_calc(map.beatmap_id, &score.enabled_mods, None)?;
                    let max_pp = parse_pp_calc(max_pp_child)?;
                    if map.approval_status == ApprovalStatus::Ranked
                        || map.approval_status == ApprovalStatus::Loved
                    {
                        // Insert max pp value into database
                        let data = ctx.data.read();
                        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                        if map_in_db {
                            mysql.update_mania_pp_map(
                                map.beatmap_id,
                                &score.enabled_mods,
                                max_pp,
                            )?;
                        } else {
                            mysql.insert_mania_pp_map(
                                map.beatmap_id,
                                &score.enabled_mods,
                                max_pp,
                            )?;
                        }
                    }
                    max_pp
                };
                let stars = if let Some(stars) = stars {
                    stars
                } else {
                    let stars = calc_stars(map.beatmap_id, &score.enabled_mods)?;
                    mem::drop(lock);
                    if map.approval_status == ApprovalStatus::Ranked
                        || map.approval_status == ApprovalStatus::Loved
                    {
                        // Insert stars value into database
                        let data = ctx.data.read();
                        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                        if stars_in_db {
                            mysql.update_mania_stars_map(
                                map.beatmap_id,
                                &score.enabled_mods,
                                stars,
                            )?;
                        } else {
                            mysql.insert_mania_stars_map(
                                map.beatmap_id,
                                &score.enabled_mods,
                                stars,
                            )?;
                        }
                    }
                    stars
                };
                Ok(Self::Mania { pp, max_pp, stars })
            }
            GameMode::CTB => Ok(Self::Fruits { stars: map.stars }),
        }
    }

    pub fn calculate_oppai_pp<S>(score: &S, map: &Beatmap) -> Result<f32, Error>
    where
        S: SubScore,
    {
        let mut oppai = Oppai::new();
        if !score.mods().is_empty() {
            let bits = score.mods().as_bits();
            oppai.set_mods(bits);
        }
        let map_path = osu::prepare_beatmap_file(map.beatmap_id)?;
        oppai
            .set_miss_count(score.miss())
            .set_hits(score.c100(), score.c50())
            .set_end_index(score.hits(map.mode))
            .set_combo(score.combo())
            .calculate(Some(&map_path))?;
        Ok(oppai.get_pp())
    }

    pub fn calculate_mania_pp<S>(score: &S, map: &Beatmap, ctx: &Context) -> Result<f32, Error>
    where
        S: SubScore,
    {
        let mods = &score.mods();
        let half_score = half_score(mods);
        if score.grade() == Grade::F || score.score() < half_score {
            Ok(0.0)
        } else {
            let mutex = {
                let data = ctx.data.read();
                data.get::<PerformanceCalculatorLock>()
                    .expect("Could not get PerformanceCalculatorLock")
                    .clone()
            };
            let _ = mutex.lock();
            let child = start_pp_calc(map.beatmap_id, mods, Some(score.score()))?;
            parse_pp_calc(child)
        }
    }

    pub fn calculate_max(
        map: &Beatmap,
        mods: &GameMods,
        ctx: Option<&Context>,
    ) -> Result<f32, Error> {
        match map.mode {
            GameMode::STD | GameMode::TKO => {
                let mut oppai = Oppai::new();
                if !mods.is_empty() {
                    let bits = mods.as_bits();
                    oppai.set_mods(bits);
                }
                let map_path = osu::prepare_beatmap_file(map.beatmap_id)?;
                Ok(oppai.calculate(Some(&map_path))?.get_pp())
            }
            GameMode::MNA => {
                let ctx = ctx.unwrap();
                // Try retrieving max pp of the map from database
                let (max_pp, map_in_db) = {
                    let data = ctx.data.read();
                    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                    match mysql.get_mania_mod_pp(map.beatmap_id, &mods) {
                        Ok(result) => (result, true),
                        Err(why) => {
                            if let Error::Custom(_) = why {
                                warn!(
                                    "Some mod bit error for mods {} in pp_mania_mods table",
                                    mods
                                );
                            }
                            (None, false)
                        }
                    }
                };
                // If max pp were found, get them
                if let Some(max_pp) = max_pp {
                    Ok(max_pp)
                // Otherwise start calculating them in new thread
                } else {
                    let max_pp = {
                        let mutex = {
                            let data = ctx.data.read();
                            data.get::<PerformanceCalculatorLock>()
                                .expect("Could not get PerformanceCalculatorLock")
                                .clone()
                        };
                        let _ = mutex.lock();
                        let max_pp_child = start_pp_calc(map.beatmap_id, mods, None)?;
                        parse_pp_calc(max_pp_child)?
                    };
                    // Insert max pp value into database
                    let data = ctx.data.read();
                    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                    if map_in_db {
                        mysql.update_mania_pp_map(map.beatmap_id, &mods, max_pp)?;
                    } else {
                        mysql.insert_mania_pp_map(map.beatmap_id, &mods, max_pp)?;
                    }
                    Ok(max_pp)
                }
            }
            GameMode::CTB => Err(Error::Custom("Cannot calculate max ctb pp".to_string())),
        }
    }

    pub fn recalculate(&mut self, score: &Score, mode: GameMode) -> Result<(), Error> {
        match self {
            Self::Oppai { oppai, pp, .. } => {
                if !score.enabled_mods.is_empty() {
                    let bits = score.enabled_mods.as_bits();
                    oppai.set_mods(bits);
                }
                oppai
                    .set_miss_count(score.count_miss)
                    .set_hits(score.count100, score.count50)
                    .set_end_index(score.total_hits(mode))
                    .set_combo(score.max_combo)
                    .calculate(None)?;
                *pp = oppai.get_pp();
                Ok(())
            }
            Self::Mania { .. } => Err(Error::Custom("Cannot recalculate mania pp".to_string())),
            Self::Fruits { .. } => Err(Error::Custom("Cannot recalculate ctb pp".to_string())),
        }
    }

    pub fn pp(&self) -> f32 {
        match self {
            Self::Oppai { pp, .. } => *pp,
            Self::Mania { pp, .. } => *pp,
            Self::Fruits { .. } => panic!("Don't call pp on ctb maps!"),
        }
    }

    pub fn max_pp(&self) -> f32 {
        match self {
            Self::Oppai { max_pp, .. } => *max_pp,
            Self::Mania { max_pp, .. } => *max_pp,
            Self::Fruits { .. } => panic!("Don't call pp_max on ctb maps!"),
        }
    }

    pub fn stars(&self) -> f32 {
        match self {
            Self::Oppai { stars, .. } => *stars,
            Self::Mania { stars, .. } => *stars,
            Self::Fruits { stars, .. } => *stars,
        }
    }

    pub fn oppai(&self) -> Option<&Oppai> {
        match self {
            Self::Oppai { oppai, .. } => Some(oppai),
            _ => None,
        }
    }
}

fn start_pp_calc(map_id: u32, mods: &GameMods, score: Option<u32>) -> Result<Child, Error> {
    let map_path = osu::prepare_beatmap_file(map_id)?;
    let mut cmd = Command::new("dotnet");
    cmd.arg(env::var("PERF_CALC").unwrap())
        .arg("simulate")
        .arg("mania")
        .arg(map_path);
    for &m in mods.iter() {
        cmd.arg("-m").arg(m.to_string());
    }
    cmd.arg("-s");
    if let Some(score) = score {
        cmd.arg(score.to_string());
    } else {
        cmd.arg(max_score(mods).to_string());
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(Error::from)
}

fn parse_pp_calc(child: Child) -> Result<f32, Error> {
    let output = child.wait_with_output()?;
    if output.status.success() {
        let result = String::from_utf8(output.stdout)
            .map_err(|_| Error::Custom("Could not read stdout string".to_string()))?;
        f32::from_str(&result.trim())
            .map_err(|_| Error::Custom("PerfCalc result could not be parsed into pp".to_string()))
    } else {
        let error_msg = String::from_utf8(output.stderr)
            .map_err(|_| Error::Custom("Could not read stderr string".to_string()))?;
        Err(Error::Custom(error_msg))
    }
}

fn calc_stars(map_id: u32, mods: &GameMods) -> Result<f32, Error> {
    let map_path = osu::prepare_beatmap_file(map_id)?;
    let mut cmd = Command::new("dotnet");
    cmd.arg(env::var("PERF_CALC").unwrap())
        .arg("difficulty")
        .arg(map_path);
    for &m in mods.iter() {
        cmd.arg("-m").arg(m.to_string());
    }
    let output = cmd.output()?;
    if output.status.success() {
        let result = String::from_utf8(output.stdout)
            .map_err(|_| Error::Custom("Could not read stdout string".to_string()))?;
        f32::from_str(&result.trim()).map_err(|_| {
            Error::Custom("PerfCalc result could not be parsed into stars".to_string())
        })
    } else {
        let error_msg = String::from_utf8(output.stderr)
            .map_err(|_| Error::Custom("Could not read stderr string".to_string()))?;
        Err(Error::Custom(error_msg))
    }
}

fn adjusted_score(mut score: f32, mods: &GameMods) -> u32 {
    if mods.contains(&GameMod::NoFail) {
        score /= 2.0;
    }
    if mods.contains(&GameMod::Easy) {
        score /= 2.0;
    }
    if mods.contains(&GameMod::HalfTime) {
        score /= 2.0;
    }
    score as u32
}

fn half_score(mods: &GameMods) -> u32 {
    adjusted_score(500_000.0, mods)
}

fn max_score(mods: &GameMods) -> u32 {
    adjusted_score(1_000_000.0, mods)
}

pub trait SubScore {
    fn miss(&self) -> u32;
    fn c50(&self) -> u32;
    fn c100(&self) -> u32;
    fn c300(&self) -> u32;
    fn combo(&self) -> u32;
    fn mods(&self) -> &GameMods;
    fn hits(&self, mode: GameMode) -> u32;
    fn grade(&self) -> Grade;
    fn score(&self) -> u32;
}

impl SubScore for Score {
    fn miss(&self) -> u32 {
        self.count_miss
    }
    fn c50(&self) -> u32 {
        self.count50
    }
    fn c100(&self) -> u32 {
        self.count100
    }
    fn c300(&self) -> u32 {
        self.count300
    }
    fn combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> &GameMods {
        &self.enabled_mods
    }
    fn hits(&self, mode: GameMode) -> u32 {
        self.total_hits(mode)
    }
    fn grade(&self) -> Grade {
        self.grade
    }
    fn score(&self) -> u32 {
        self.score
    }
}

impl SubScore for ScraperScore {
    fn miss(&self) -> u32 {
        self.count_miss
    }
    fn c50(&self) -> u32 {
        self.count50
    }
    fn c100(&self) -> u32 {
        self.count100
    }
    fn c300(&self) -> u32 {
        self.count300
    }
    fn combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> &GameMods {
        &self.enabled_mods
    }
    fn hits(&self, _: GameMode) -> u32 {
        self.total_hits()
    }
    fn grade(&self) -> Grade {
        self.grade
    }
    fn score(&self) -> u32 {
        self.score
    }
}

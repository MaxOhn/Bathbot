use crate::{
    database::MySQL, roppai::Oppai, scraper::ScraperScore, util::osu, Error,
    PerformanceCalculatorLock,
};

use rosu::models::{Beatmap, GameMod, GameMode, GameMods, Grade, Score};
use serenity::prelude::Context;
use std::{
    env, mem,
    process::{Child, Command, Stdio},
    str::FromStr,
};

pub enum PPProvider {
    #[allow(dead_code)] // Bug in rust compiler, remove when bug is fixed [22.2.2020]
    Oppai { oppai: Oppai, pp: f32, max_pp: f32 },
    #[allow(dead_code)]
    Mania { pp: f32, max_pp: f32 },
    #[allow(dead_code)]
    Fruits,
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
                Ok(Self::Oppai { oppai, pp, max_pp })
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
                    mem::drop(lock);
                    max_pp
                // Otherwise start calculating them in new thread
                } else {
                    let max_pp_child = start_pp_calc(map.beatmap_id, &score.enabled_mods, None)?;
                    let max_pp = parse_pp_calc(max_pp_child)?;
                    mem::drop(lock);
                    // Insert max pp value into database
                    let data = ctx.data.read();
                    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                    if map_in_db {
                        mysql.update_mania_pp_map(map.beatmap_id, &score.enabled_mods, max_pp)?;
                    } else {
                        mysql.insert_mania_pp_map(map.beatmap_id, &score.enabled_mods, max_pp)?;
                    }
                    max_pp
                };
                Ok(Self::Mania { pp, max_pp })
            }
            GameMode::CTB => Ok(Self::Fruits),
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
            Self::Fruits => Err(Error::Custom("Cannot recalculate ctb pp".to_string())),
        }
    }

    pub fn pp(&self) -> f32 {
        match self {
            Self::Oppai { pp, .. } => *pp,
            Self::Mania { pp, .. } => *pp,
            Self::Fruits => panic!("Don't call pp on ctb maps!"),
        }
    }

    pub fn max_pp(&self) -> f32 {
        match self {
            Self::Oppai { max_pp, .. } => *max_pp,
            Self::Mania { max_pp, .. } => *max_pp,
            Self::Fruits => panic!("Don't call pp_max on ctb maps!"),
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
    let cmd_str = format!(
        "dotnet {} simulate mania {}",
        env::var("PERF_CALC").unwrap(),
        map_path
    );
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C");
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        cmd
    };
    cmd.arg(cmd_str);
    for &m in mods.iter() {
        cmd.arg("-m").arg(m.to_string());
    }
    if let Some(score) = score {
        cmd.arg("-s").arg(score.to_string());
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
        let line = result
            .lines()
            .last()
            .ok_or_else(|| Error::Custom("stdout string was empty".to_string()))?;
        f32::from_str(line.split(':').last().unwrap().trim()).map_err(|_| {
            Error::Custom("Right side of last line did not contain an f32".to_string())
        })
    } else {
        let error_msg = String::from_utf8(output.stderr)
            .map_err(|_| Error::Custom("Could not read stderr string".to_string()))?;
        Err(Error::Custom(error_msg))
    }
}

fn half_score(mods: &GameMods) -> u32 {
    let mut half_score = 500_000.0;
    if mods.contains(&GameMod::NoFail) {
        half_score /= 2.0;
    }
    if mods.contains(&GameMod::Easy) {
        half_score /= 2.0;
    }
    if mods.contains(&GameMod::HalfTime) {
        half_score /= 2.0;
    }
    half_score as u32
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

use crate::{database::MySQL, util::osu, Error, PerformanceCalculatorLock};

use roppai::Oppai;
use rosu::models::{Beatmap, GameMode, GameMods, Score};
use serenity::prelude::Context;
use std::{
    mem,
    process::{Child, Command, Stdio},
    str::FromStr,
};

const PP_MANIA_CMD: &str =
    "dotnet run --project osu-tools/PerformanceCalculator/ -- simulate mania ";

pub enum PPProvider {
    #[allow(dead_code)] // Bug in rust compiler, remove when bug is fixed [22.2.2020]
    Oppai { oppai: Oppai, pp: f32, max_pp: f32 },
    #[allow(dead_code)]
    Mania { pp: f32, max_pp: f32 },
    #[allow(dead_code)]
    Fruits,
}

impl PPProvider {
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
                // Start calculating pp of the score in new thread
                let (pp_child, lock) = if score.pp.is_none() {
                    let lock = mutex.as_ref().unwrap().lock();
                    let child =
                        start_pp_calc(map.beatmap_id, &score.enabled_mods, Some(score.score))?;
                    (Some(child), Some(lock))
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
                } else {
                    score.pp.unwrap()
                };
                // If max pp were found, get them
                let max_pp = if let Some(max_pp) = max_pp {
                    /*
                    debug!(
                        "Found max pp for map id {} and mods {}",
                        map.beatmap_id, score.enabled_mods
                    );
                    */
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
    let cmd_str = format!("{}{}", PP_MANIA_CMD, map_path);
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(cmd_str);
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

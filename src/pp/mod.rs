pub mod roppai;

use roppai::Oppai;

use crate::{
    util::{error::PPError, osu::prepare_beatmap_file, BeatmapExt, ScoreExt},
    BotResult, Context, CONFIG,
};

use bitflags::bitflags;
use rosu::model::{
    ApprovalStatus::{self, Approved, Loved, Ranked},
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

#[derive(Default)]
pub struct PPCalculator {
    count_300: Option<u32>,
    count_100: Option<u32>,
    count_50: Option<u32>,
    count_miss: Option<u32>,
    max_combo_score: Option<u32>,
    mods: Option<GameMods>,
    score: Option<u32>,

    map_id: Option<u32>,
    mode: Option<GameMode>,
    max_combo_map: Option<u32>,
    default_stars: Option<f32>,
    approval_status: Option<ApprovalStatus>,

    pp: Option<f32>,
    max_pp: Option<f32>,
    stars: Option<f32>,
}

impl PPCalculator {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn score(mut self, score: impl ScoreExt) -> Self {
        self.count_300 = Some(score.count_300());
        self.count_100 = Some(score.count_100());
        self.count_50 = Some(score.count_50());
        self.count_miss = Some(score.count_miss());
        self.max_combo_score = Some(score.max_combo());
        self.mods = Some(score.mods());
        self.score = Some(score.score());
        self.pp = score.pp();
        self
    }
    pub fn map(mut self, map: impl BeatmapExt) -> Self {
        self.map_id = Some(map.map_id());
        self.max_combo_map = map.max_combo();
        self.mode = Some(map.mode());
        self.default_stars = map.stars();
        self.approval_status = Some(map.approval_status());
        self
    }
    fn total_hits_oppai(&self) -> Option<u32> {
        let mut amount = self.count_300? + self.count_100? + self.count_miss?;
        if self.mode? == GameMode::STD {
            amount += self.count_50?;
        }
        Some(amount)
    }
    pub async fn calculate(
        &mut self,
        calculations: Calculations,
        ctx: Option<&Context>,
    ) -> BotResult<()> {
        let map_path = match self.map_id {
            Some(map_id) => prepare_beatmap_file(map_id).await?,
            None => return Err(PPError::NoMapId.into()),
        };
        let (pp, max_pp, stars) = tokio::join!(
            calculate_pp(&self, calculations, &map_path, ctx),
            calculate_max_pp(&self, calculations, &map_path, ctx),
            calculate_stars(&self, calculations, &map_path, ctx)
        );
        if let Ok(Some(pp)) = pp {
            self.pp = Some(pp);
        }
        if let Ok(Some(max_pp)) = max_pp {
            self.max_pp = Some(max_pp);
        }
        if let Ok(Some(stars)) = stars {
            self.stars = Some(stars);
        }
        pp.map_err(|e| PPError::PP(Box::new(e)))?;
        max_pp.map_err(|e| PPError::MaxPP(Box::new(e)))?;
        stars.map_err(|e| PPError::Stars(Box::new(e)))?;
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
}

async fn calculate_pp(
    data: &PPCalculator,
    calculations: Calculations,
    map_path: &str,
    ctx: Option<&Context>,
) -> Result<Option<f32>, PPError> {
    // Do we want to calculate pp?
    if !calculations.contains(Calculations::PP) {
        return Ok(None);
    }
    // Is pp value already present?
    if data.pp.is_some() {
        return Ok(data.pp);
    }
    let mode = data.mode.unwrap();
    // Distinguish between mods
    let pp = match mode {
        // Oppai for STD and TKO
        GameMode::STD | GameMode::TKO => {
            let mut oppai = Oppai::new();
            if let Some(mods) = data.mods {
                oppai.set_mods(mods.bits());
            }
            if let Some(count_miss) = data.count_miss {
                oppai.set_miss_count(count_miss);
            }
            if let Some(count_100) = data.count_100 {
                if let Some(count_50) = data.count_50 {
                    oppai.set_hits(count_100, count_50);
                }
            }
            if let Some(max_combo) = data.max_combo_score {
                oppai.set_combo(max_combo);
            }
            if let Some(total_hits) = data.total_hits_oppai() {
                oppai.set_end_index(total_hits);
            }
            oppai.calculate(map_path)?.get_pp()
        }
        // osu-tools for MNA and CTB
        GameMode::MNA | GameMode::CTB => {
            let ctx = ctx.ok_or(PPError::NoContext(mode))?;
            let mut cmd = Command::new("dotnet");
            cmd.kill_on_drop(true)
                .arg(CONFIG.get().unwrap().perf_calc_path.as_os_str())
                .arg("simulate");
            match mode {
                GameMode::MNA => cmd.arg("mania"),
                GameMode::CTB => cmd.arg("catch"),
                _ => unreachable!(),
            };
            cmd.arg(map_path);
            if let Some(mods) = data.mods {
                if !mods.is_empty() {
                    for m in mods.iter().filter(|&m| m != GameMods::ScoreV2) {
                        cmd.arg("-m").arg(m.to_string());
                    }
                }
            }
            if mode == GameMode::MNA {
                if let Some(score) = data.score {
                    cmd.arg("-s").arg(score.to_string());
                }
            } else if mode == GameMode::CTB {
                if let Some(combo) = data.max_combo_score {
                    cmd.arg("-c").arg(combo.to_string());
                }
                if let Some(misses) = data.count_miss {
                    cmd.arg("-X").arg(misses.to_string());
                }
                if let Some(count_100) = data.count_100 {
                    cmd.arg("-D").arg(count_100.to_string());
                }
                if let Some(count_50) = data.count_50 {
                    cmd.arg("-T").arg(count_50.to_string());
                }
            }
            parse_calculation(cmd, ctx).await?
        }
    };
    Ok(Some(pp))
}

async fn calculate_max_pp(
    data: &PPCalculator,
    calculations: Calculations,
    map_path: &str,
    ctx: Option<&Context>,
) -> Result<Option<f32>, PPError> {
    // Do we want to calculate max pp?
    if !calculations.contains(Calculations::MAX_PP) {
        return Ok(None);
    }
    let map_id = data.map_id.unwrap();
    let mode = data.mode.unwrap();
    // Distinguish between mods
    match mode {
        // Oppai for STD and TKO
        GameMode::STD | GameMode::TKO => {
            let mut oppai = Oppai::new();
            if let Some(mods) = data.mods {
                oppai.set_mods(mods.bits());
            }
            Ok(Some(oppai.calculate(map_path)?.get_pp()))
        }
        // osu-tools for MNA and CTB
        GameMode::MNA | GameMode::CTB => {
            let ctx = ctx.ok_or(PPError::NoContext(mode))?;
            // Is value already stored?
            let mods = data.mods.unwrap_or_default();
            let stored = ctx.pp(mode);
            if let Some(max_pp) = stored
                .get(&map_id)
                .and_then(|mod_map| mod_map.get(&mods).map(|(max_pp, _)| *max_pp))
            {
                return Ok(Some(max_pp));
            }
            // If not, calculate
            let mut cmd = Command::new("dotnet");
            cmd.kill_on_drop(true)
                .arg(CONFIG.get().unwrap().perf_calc_path.as_os_str())
                .arg("simulate");
            match mode {
                GameMode::MNA => cmd.arg("mania"),
                GameMode::CTB => cmd.arg("catch"),
                _ => unreachable!(),
            };
            cmd.arg(map_path);
            if let Some(mods) = data.mods {
                if !mods.is_empty() {
                    for m in mods.iter().filter(|&m| m != GameMods::ScoreV2) {
                        cmd.arg("-m").arg(m.to_string());
                    }
                }
            }
            if mode == GameMode::MNA {
                if let Some(mods) = data.mods {
                    cmd.arg("-s")
                        .arg(((1_000_000.0 * mods.score_multiplier(mode)) as u32).to_string());
                }
            }
            let max_pp = parse_calculation(cmd, ctx).await?;
            // Store value
            if let Ranked | Loved | Approved = data.approval_status.unwrap() {
                let mods = data.mods.unwrap_or_default();
                ctx.pp(mode)
                    .entry(map_id)
                    .and_modify(|value_map| {
                        value_map.insert(mods, (max_pp, true));
                    })
                    .or_insert_with(|| {
                        let mut value_map = HashMap::new();
                        value_map.insert(mods, (max_pp, true));
                        value_map
                    });
            }
            Ok(Some(max_pp))
        }
    }
}

async fn calculate_stars(
    data: &PPCalculator,
    calculations: Calculations,
    map_path: &str,
    ctx: Option<&Context>,
) -> Result<Option<f32>, PPError> {
    if !calculations.contains(Calculations::STARS) {
        return Ok(None);
    }
    if let Some(mode) = data.mode {
        if let Some(mods) = data.mods {
            if !mods.changes_stars(mode) && data.default_stars.is_some() {
                return Ok(data.default_stars);
            }
        }
    }
    let mode = data.mode.unwrap();
    let map_id = data.map_id.unwrap();
    match mode {
        GameMode::STD | GameMode::TKO => {
            let mut oppai = Oppai::new();
            if let Some(mods) = data.mods {
                oppai.set_mods(mods.bits());
            }
            Ok(Some(oppai.calculate(map_path)?.get_stars()))
        }
        GameMode::MNA | GameMode::CTB => {
            let ctx = ctx.ok_or(PPError::NoContext(mode))?;
            // Is value already stored?
            let mods = data.mods.unwrap_or_default();
            let stored = ctx.stars(mode);
            if let Some(stars) = stored
                .get(&map_id)
                .and_then(|mod_map| mod_map.get(&mods).map(|(stars, _)| *stars))
            {
                return Ok(Some(stars));
            }
            let mut cmd = Command::new("dotnet");
            cmd.kill_on_drop(true)
                .arg(CONFIG.get().unwrap().perf_calc_path.as_os_str())
                .arg("difficulty")
                .arg(map_path);
            if let Some(mods) = data.mods {
                if !mods.is_empty() {
                    for m in mods.iter().filter(|&m| m != GameMods::ScoreV2) {
                        cmd.arg("-m").arg(m.to_string());
                    }
                }
            }
            let stars = parse_calculation(cmd, ctx).await?;
            // Store value
            if let Ranked | Loved | Approved = data.approval_status.unwrap() {
                let mods = data.mods.unwrap_or_default();
                ctx.stars(mode)
                    .entry(map_id)
                    .and_modify(|value_map| {
                        value_map.insert(mods, (stars, true));
                    })
                    .or_insert_with(|| {
                        let mut value_map = HashMap::new();
                        value_map.insert(mods, (stars, true));
                        value_map
                    });
            }
            Ok(Some(stars))
        }
    }
}

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

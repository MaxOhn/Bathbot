use crate::{
    database::MySQL,
    roppai::Oppai,
    scraper::{OsuStatsMap, OsuStatsScore, ScraperScore},
    util::osu,
    PerformanceCalculatorLock,
};

use failure::Error;
use rosu::models::{
    ApprovalStatus::{self, Approved, Loved, Ranked},
    Beatmap, GameMode, GameMods, Grade, Score,
};
use serenity::prelude::{RwLock, TypeMap};
use std::{env, str::FromStr, sync::Arc};
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

    data: Option<Arc<RwLock<TypeMap>>>,

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
    pub fn data(mut self, data: Arc<RwLock<TypeMap>>) -> Self {
        self.data = Some(data);
        self
    }
    fn total_hits_oppai(&self) -> Option<u32> {
        let mut amount = self.count_300? + self.count_100? + self.count_miss?;
        if self.mode? == GameMode::STD {
            amount += self.count_50?;
        }
        Some(amount)
    }
    pub async fn calculate(&mut self, calculations: Calculations) -> Result<(), Error> {
        let map_path = match self.map_id {
            Some(map_id) => osu::prepare_beatmap_file(map_id).await?,
            None => bail!("Cannot calculate without map_id"),
        };
        let (pp, max_pp, stars) = tokio::join!(
            calculate_pp(&self, calculations, &map_path),
            calculate_max_pp(&self, calculations, &map_path),
            calculate_stars(&self, calculations, &map_path)
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
        pp.map_err(|why| format_err!("Error calculating pp: {}", why))?;
        max_pp.map_err(|why| format_err!("Error calculating max pp: {}", why))?;
        stars.map_err(|why| format_err!("Error calculating stars: {}", why))?;
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
) -> Result<Option<f32>, Error> {
    // Do we want to calculate pp?
    if !calculations.contains(Calculations::PP) {
        return Ok(None);
    }
    // Is pp value already present?
    if data.pp.is_some() {
        return Ok(data.pp);
    }
    let mode = match data.mode {
        Some(mode) => mode,
        None => bail!("Cannot calculate pp without mode"),
    };
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
            oppai.calculate(Some(map_path))?;
            oppai.get_pp()
        }
        // osu-tools for MNA and CTB
        GameMode::MNA | GameMode::CTB => {
            let data_map = match data.data {
                Some(ref data_map) => data_map,
                None => bail!("Cannot calculate {} pp without typemap", mode),
            };
            let mut cmd = Command::new("dotnet");
            cmd.kill_on_drop(true)
                .arg(env::var("PERF_CALC").unwrap())
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
            parse_calculation(cmd, data_map).await?
        }
    };
    Ok(Some(pp))
}

async fn calculate_max_pp(
    data: &PPCalculator,
    calculations: Calculations,
    map_path: &str,
) -> Result<Option<f32>, Error> {
    // Do we want to calculate max pp?
    if !calculations.contains(Calculations::MAX_PP) {
        return Ok(None);
    }
    let mode = match data.mode {
        Some(mode) => mode,
        None => bail!("Cannot calculate pp without mode"),
    };
    let map_id = match data.map_id {
        Some(map_id) => map_id,
        None => bail!("Cannot calculate pp without map_id"),
    };
    // Distinguish between mods
    match mode {
        // Oppai for STD and TKO
        GameMode::STD | GameMode::TKO => {
            let mut oppai = Oppai::new();
            if let Some(mods) = data.mods {
                oppai.set_mods(mods.bits());
            }
            Ok(Some(oppai.calculate(Some(map_path))?.get_pp()))
        }
        // osu-tools for MNA and CTB
        GameMode::MNA | GameMode::CTB => {
            let data_map = match data.data {
                Some(ref data_map) => data_map,
                None => bail!("Cannot calculate {} max pp without typemap", mode),
            };
            // Is value already stored in DB?
            {
                let data_map = data_map.read().await;
                let mysql = data_map.get::<MySQL>().unwrap();
                let mods = data.mods.unwrap_or_default();
                if let Ok(Some(max_pp)) = mysql.get_mod_pp(map_id, mode, mods).await {
                    return Ok(Some(max_pp));
                }
            }
            // If not, calculate
            let mut cmd = Command::new("dotnet");
            cmd.kill_on_drop(true)
                .arg(env::var("PERF_CALC").unwrap())
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
            let max_pp = parse_calculation(cmd, data_map).await?;
            // Store value in DB
            if let Ranked | Loved | Approved = data.approval_status.unwrap() {
                let data_map = data_map.read().await;
                let mysql = data_map.get::<MySQL>().unwrap();
                let mods = data.mods.unwrap_or_default();
                if let Err(why) = mysql.insert_pp_map(map_id, mode, mods, max_pp).await {
                    warn!("Error while inserting max pp: {}", why);
                }
            }
            Ok(Some(max_pp))
        }
    }
}

async fn calculate_stars(
    data: &PPCalculator,
    calculations: Calculations,
    map_path: &str,
) -> Result<Option<f32>, Error> {
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
    let mode = match data.mode {
        Some(mode) => mode,
        None => bail!("Cannot calculate stars without mode"),
    };
    let map_id = match data.map_id {
        Some(map_id) => map_id,
        None => bail!("Cannot calculate stars without map_id"),
    };
    match mode {
        GameMode::STD | GameMode::TKO => {
            let mut oppai = Oppai::new();
            if let Some(mods) = data.mods {
                oppai.set_mods(mods.bits());
            }
            Ok(Some(oppai.calculate(Some(map_path))?.get_stars()))
        }
        GameMode::MNA | GameMode::CTB => {
            let data_map = match data.data {
                Some(ref data_map) => data_map,
                None => bail!("Cannot calculate {} stars without typemap", mode),
            };
            // Is value already stored in DB?
            {
                let data_map = data_map.read().await;
                let mysql = data_map.get::<MySQL>().unwrap();
                let mods = data.mods.unwrap_or_default();
                if let Ok(Some(stars)) = mysql.get_mod_stars(map_id, mode, mods).await {
                    return Ok(Some(stars));
                }
            }
            let mut cmd = Command::new("dotnet");
            cmd.kill_on_drop(true)
                .arg(env::var("PERF_CALC").unwrap())
                .arg("difficulty")
                .arg(map_path);
            if let Some(mods) = data.mods {
                if !mods.is_empty() {
                    for m in mods.iter().filter(|&m| m != GameMods::ScoreV2) {
                        cmd.arg("-m").arg(m.to_string());
                    }
                }
            }
            let stars = parse_calculation(cmd, data_map).await?;
            // Store value in DB
            if let Ranked | Loved | Approved = data.approval_status.unwrap() {
                let data_map = data_map.read().await;
                let mysql = data_map.get::<MySQL>().unwrap();
                let mods = data.mods.unwrap_or_default();
                if let Err(why) = mysql.insert_stars_map(map_id, mode, mods, stars).await {
                    warn!("Error while inserting stars: {}", why);
                }
            }
            Ok(Some(stars))
        }
    }
}

async fn parse_calculation(mut cmd: Command, data: &RwLock<TypeMap>) -> Result<f32, Error> {
    let calculation = time::timeout(time::Duration::from_secs(10), cmd.output());
    let output = {
        let data = data.read().await;
        let mutex = data.get::<PerformanceCalculatorLock>().unwrap();
        let _lock = mutex.lock().await;
        match calculation.await {
            Ok(output) => output?,
            Err(_) => bail!("Timeout while waiting for output",),
        }
    };
    if output.status.success() {
        let result = String::from_utf8(output.stdout)?;
        Ok(f32::from_str(&result.trim())?)
    } else {
        bail!(String::from_utf8(output.stderr)?)
    }
}

pub trait BeatmapExt {
    fn max_combo(&self) -> Option<u32>;
    fn map_id(&self) -> u32;
    fn mode(&self) -> GameMode;
    fn stars(&self) -> Option<f32>;
    fn approval_status(&self) -> ApprovalStatus;
}

impl BeatmapExt for &OsuStatsMap {
    fn max_combo(&self) -> Option<u32> {
        self.max_combo
    }
    fn map_id(&self) -> u32 {
        self.beatmap_id
    }
    fn mode(&self) -> GameMode {
        self.mode
    }
    fn stars(&self) -> Option<f32> {
        self.stars
    }
    fn approval_status(&self) -> ApprovalStatus {
        self.approval_status
    }
}

impl BeatmapExt for &Beatmap {
    fn max_combo(&self) -> Option<u32> {
        self.max_combo
    }
    fn map_id(&self) -> u32 {
        self.beatmap_id
    }
    fn mode(&self) -> GameMode {
        self.mode
    }
    fn stars(&self) -> Option<f32> {
        Some(self.stars)
    }
    fn approval_status(&self) -> ApprovalStatus {
        self.approval_status
    }
}

pub trait ScoreExt {
    fn count_miss(&self) -> u32;
    fn count_50(&self) -> u32;
    fn count_100(&self) -> u32;
    fn count_300(&self) -> u32;
    fn count_geki(&self) -> u32;
    fn count_katu(&self) -> u32;
    fn max_combo(&self) -> u32;
    fn mods(&self) -> GameMods;
    fn hits(&self, mode: GameMode) -> u32;
    fn grade(&self) -> Grade;
    fn score(&self) -> u32;
    fn pp(&self) -> Option<f32>;
}

impl ScoreExt for &OsuStatsScore {
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    fn count_50(&self) -> u32 {
        self.count50
    }
    fn count_100(&self) -> u32 {
        self.count100
    }
    fn count_300(&self) -> u32 {
        self.count300
    }
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> GameMods {
        self.enabled_mods
    }
    fn hits(&self, _mode: GameMode) -> u32 {
        let mut amount = self.count300 + self.count100 + self.count_miss;
        let mode = self.map.mode;
        if mode != GameMode::TKO {
            amount += self.count50;
            if mode != GameMode::STD {
                amount += self.count_katu;
                if mode != GameMode::CTB {
                    amount += self.count_geki;
                }
            }
        }
        amount
    }
    fn grade(&self) -> Grade {
        self.grade
    }
    fn score(&self) -> u32 {
        self.score
    }
    fn pp(&self) -> Option<f32> {
        self.pp
    }
}

impl ScoreExt for &Score {
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    fn count_50(&self) -> u32 {
        self.count50
    }
    fn count_100(&self) -> u32 {
        self.count100
    }
    fn count_300(&self) -> u32 {
        self.count300
    }
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> GameMods {
        self.enabled_mods
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
    fn pp(&self) -> Option<f32> {
        self.pp
    }
}

impl ScoreExt for &ScraperScore {
    fn count_miss(&self) -> u32 {
        self.count_miss
    }
    fn count_50(&self) -> u32 {
        self.count50
    }
    fn count_100(&self) -> u32 {
        self.count100
    }
    fn count_300(&self) -> u32 {
        self.count300
    }
    fn count_geki(&self) -> u32 {
        self.count_geki
    }
    fn count_katu(&self) -> u32 {
        self.count_katu
    }
    fn max_combo(&self) -> u32 {
        self.max_combo
    }
    fn mods(&self) -> GameMods {
        self.enabled_mods
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
    fn pp(&self) -> Option<f32> {
        self.pp
    }
}

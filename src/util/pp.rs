use crate::{
    database::MySQL, roppai::Oppai, scraper::ScraperScore, util::osu, PerformanceCalculatorLock,
};

use failure::Error;
use rosu::models::{ApprovalStatus, Beatmap, GameMode, GameMods, Grade, Score};
use serenity::prelude::{RwLock, TypeMap};
use std::{env, mem, process::Stdio, str::FromStr};
use tokio::{
    process::{Child, Command},
    time,
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
        pp: f32,
        max_pp: f32,
        stars: f32,
    },
}

async fn new_oppai(score: &Score, map: &Beatmap) -> Result<PPProvider, Error> {
    let map_path = osu::prepare_beatmap_file(map.beatmap_id).await?;
    let mut oppai = Oppai::new();
    oppai.set_mods(score.enabled_mods.bits());
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
    Ok(PPProvider::Oppai {
        oppai,
        pp,
        max_pp,
        stars,
    })
}

async fn new_perf_calc<'a>(
    score: &'a Score,
    map: &Beatmap,
    data: &RwLock<TypeMap>,
) -> Result<PPProvider, Error> {
    let mode = map.mode;
    let mutex = if score.pp.is_none() {
        let data = data.read().await;
        Some(data.get::<PerformanceCalculatorLock>().unwrap().clone())
    } else {
        None
    };
    let mods = score.enabled_mods;
    let (stars, stars_in_db) = if mods.changes_stars(mode) {
        // Try retrieving stars from database
        let data = data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_mod_stars(map.beatmap_id, mode, mods) {
            Ok(result) => (result, true),
            Err(why) => {
                warn!("Error while retrieving from {} stars: {}", mode, why);
                (None, false)
            }
        }
    } else {
        (Some(map.stars), true)
    };

    // Start calculating pp of the score in new async worker
    let (pp_child, lock) = if score.pp.is_none() {
        // If its a fail or below half score, it's gonna be 0pp anyway
        if score.grade == Grade::F
            || (mode == GameMode::MNA
                && score.score < (500_000.0 * mods.score_multiplier(mode)) as u32)
        {
            (None, None)
        } else {
            let lock = mutex.as_ref().unwrap().lock();
            let params = if mode == GameMode::MNA {
                CalcParam::mania(Some(score.score), mods)
            } else {
                CalcParam::ctb(score)
            };
            let child = start_pp_calc(map.beatmap_id, params).await?;
            (Some(child), Some(lock))
        }
    } else {
        (None, None)
    };
    // Try retrieving max pp of the map from database
    let (max_pp, map_in_db) = {
        let data = data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_mod_pp(map.beatmap_id, mode, mods) {
            Ok(result) => (result, true),
            Err(why) => {
                warn!(
                    "Mod bit error for mods {} in {} pp table: {}",
                    mods, mode, why
                );
                (None, false)
            }
        }
    };
    // Wait for score pp calculation to finish
    let pp = if let Some(pp_child) = pp_child {
        parse_pp_calc(pp_child).await?
    } else if score.grade == Grade::F
        || (mode == GameMode::MNA && score.score < (500_000.0 * mods.score_multiplier(mode)) as u32)
    {
        0.0
    } else {
        score.pp.unwrap()
    };
    // If max pp were found, get them
    let max_pp = if let Some(max_pp) = max_pp {
        max_pp
    // Otherwise start calculating them in new async worker
    } else {
        let params: CalcParam<'a, Score> = if mode == GameMode::MNA {
            CalcParam::mania(None, mods)
        } else {
            CalcParam::max_ctb(mods)
        };
        let max_pp_child = start_pp_calc(map.beatmap_id, params).await?;
        let max_pp = parse_pp_calc(max_pp_child).await?;
        if map.approval_status == ApprovalStatus::Ranked
            || map.approval_status == ApprovalStatus::Loved
        {
            // Insert max pp value into database
            let data = data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            f32_to_db(map_in_db, mysql, map.beatmap_id, mode, mods, max_pp, false)?;
        }
        max_pp
    };
    let stars = if let Some(stars) = stars {
        stars
    } else {
        let stars = calc_stars(map.beatmap_id, score.enabled_mods).await?;
        mem::drop(lock);
        if map.approval_status == ApprovalStatus::Ranked
            || map.approval_status == ApprovalStatus::Loved
        {
            // Insert stars value into database
            let data = data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            f32_to_db(
                stars_in_db,
                mysql,
                map.beatmap_id,
                mode,
                score.enabled_mods,
                stars,
                true,
            )?;
        }
        stars
    };
    Ok(PPProvider::Mania { pp, max_pp, stars })
}

impl PPProvider {
    pub async fn new(
        score: &Score,
        map: &Beatmap,
        data: Option<&RwLock<TypeMap>>,
    ) -> Result<Self, Error> {
        match map.mode {
            GameMode::STD | GameMode::TKO => new_oppai(score, map).await,
            GameMode::MNA => match data {
                Some(data) => new_perf_calc(score, map, data).await,
                None => bail!("Cannot calculate mania pp without TypeMap"),
            },
            GameMode::CTB => match data {
                Some(data) => new_perf_calc(score, map, data).await,
                None => bail!("Cannot calculate ctb pp without TypeMap"),
            },
        }
    }

    pub async fn calculate_oppai_pp<S>(score: &S, map: &Beatmap) -> Result<f32, Error>
    where
        S: SubScore,
    {
        let map_path = osu::prepare_beatmap_file(map.beatmap_id).await?;
        let mut oppai = Oppai::new();
        if !score.mods().is_empty() {
            let bits = score.mods().bits();
            oppai.set_mods(bits);
        }
        oppai
            .set_miss_count(score.miss())
            .set_hits(score.c100(), score.c50())
            .set_end_index(score.hits(map.mode))
            .set_combo(score.combo())
            .calculate(Some(&map_path))?;
        Ok(oppai.get_pp())
    }

    pub async fn calculate_pp<S>(
        score: &S,
        map: &Beatmap,
        data: &RwLock<TypeMap>,
    ) -> Result<f32, Error>
    where
        S: SubScore,
    {
        let mods = score.mods();
        if score.grade() == Grade::F
            || (map.mode == GameMode::MNA
                && score.score() < (500_000.0 * mods.score_multiplier(map.mode)) as u32)
        {
            Ok(0.0)
        } else {
            let mutex = {
                let data = data.read().await;
                data.get::<PerformanceCalculatorLock>().unwrap().clone()
            };
            let _ = mutex.lock();
            let params = if map.mode == GameMode::MNA {
                CalcParam::mania(Some(score.score()), mods)
            } else {
                CalcParam::ctb(score)
            };
            let child = start_pp_calc(map.beatmap_id, params).await?;
            parse_pp_calc(child).await
        }
    }

    pub async fn calculate_max<'a>(
        map: &Beatmap,
        mods: GameMods,
        data: Option<&RwLock<TypeMap>>,
    ) -> Result<f32, Error> {
        match map.mode {
            GameMode::STD | GameMode::TKO => {
                let map_path = osu::prepare_beatmap_file(map.beatmap_id).await?;
                let mut oppai = Oppai::new();
                if !mods.is_empty() {
                    let bits = mods.bits();
                    oppai.set_mods(bits);
                }
                Ok(oppai.calculate(Some(&map_path))?.get_pp())
            }
            GameMode::MNA | GameMode::CTB => {
                let data = data.unwrap();
                // Try retrieving max pp of the map from database
                let (max_pp, map_in_db) = {
                    let data = data.read().await;
                    let mysql = data.get::<MySQL>().unwrap();
                    match mysql.get_mod_pp(map.beatmap_id, map.mode, mods) {
                        Ok(result) => (result, true),
                        Err(why) => {
                            warn!("Error getting mod pp from table: {}", why);
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
                            let data = data.read().await;
                            data.get::<PerformanceCalculatorLock>().unwrap().clone()
                        };
                        let _ = mutex.lock();
                        let params: CalcParam<'a, Score> = if map.mode == GameMode::MNA {
                            CalcParam::mania(None, mods)
                        } else {
                            CalcParam::max_ctb(mods)
                        };
                        let max_pp_child = start_pp_calc(map.beatmap_id, params).await?;
                        parse_pp_calc(max_pp_child).await?
                    };
                    // Insert max pp value into database
                    let data = data.read().await;
                    let mysql = data.get::<MySQL>().unwrap();
                    if map_in_db {
                        mysql.update_pp_map(map.beatmap_id, map.mode, mods, max_pp)?;
                    } else {
                        mysql.insert_pp_map(map.beatmap_id, map.mode, mods, max_pp)?;
                    }
                    Ok(max_pp)
                }
            }
        }
    }

    pub fn recalculate(&mut self, score: &Score, mode: GameMode) -> Result<(), Error> {
        match self {
            Self::Oppai { oppai, pp, .. } => {
                oppai
                    .set_mods(score.enabled_mods.bits())
                    .set_miss_count(score.count_miss)
                    .set_hits(score.count100, score.count50)
                    .set_end_index(score.total_hits(mode))
                    .set_combo(score.max_combo)
                    .calculate(None)?;
                *pp = oppai.get_pp();
                Ok(())
            }
            Self::Mania { .. } => bail!("Cannot recalculate mania pp"),
            Self::Fruits { .. } => bail!("Cannot recalculate ctb pp"),
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

async fn start_pp_calc<S: SubScore>(map_id: u32, params: CalcParam<'_, S>) -> Result<Child, Error> {
    let map_path = osu::prepare_beatmap_file(map_id).await?;
    let mut cmd = Command::new("dotnet");
    cmd.kill_on_drop(true)
        .arg(env::var("PERF_CALC").unwrap())
        .arg("simulate");
    match params.mode() {
        GameMode::MNA => cmd.arg("mania"),
        GameMode::CTB => cmd.arg("catch"),
        _ => bail!(
            "Only use start_pp_calc for mania or ctb, not {}",
            params.mode()
        ),
    };
    cmd.arg(map_path);
    if !params.mods().is_empty() {
        for m in params.mods().iter().filter(|&m| m != GameMods::ScoreV2) {
            cmd.arg("-m").arg(m.to_string());
        }
    }
    if let CalcParam::MNA { score, mods } = params {
        cmd.arg("-s");
        if let Some(score) = score {
            cmd.arg(score.to_string());
        } else {
            cmd.arg(((1_000_000.0 * mods.score_multiplier(GameMode::MNA)) as u32).to_string());
        }
    } else if let CalcParam::CTB { score } = params {
        cmd.arg("-c").arg(score.combo().to_string());
        cmd.arg("-X").arg(score.miss().to_string());
        cmd.arg("-D").arg(score.c100().to_string());
        cmd.arg("-T").arg(score.c50().to_string());
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(Error::from)
}

async fn parse_pp_calc(child: Child) -> Result<f32, Error> {
    let calculation = time::timeout(time::Duration::from_secs(10), child.wait_with_output());
    let output = match calculation.await {
        Ok(output) => output?,
        Err(_) => bail!("Timeout while waiting for pp output",),
    };
    if output.status.success() {
        let result = String::from_utf8(output.stdout)
            .map_err(|_| format_err!("Could not read stdout string"))?;
        f32::from_str(&result.trim())
            .map_err(|_| format_err!("PerfCalc result could not be parsed into pp"))
    } else {
        let error_msg = String::from_utf8(output.stderr)
            .map_err(|_| format_err!("Could not read stderr string"))?;
        bail!(error_msg)
    }
}

async fn calc_stars(map_id: u32, mods: GameMods) -> Result<f32, Error> {
    let map_path = osu::prepare_beatmap_file(map_id).await?;
    let mut cmd = Command::new("dotnet");
    cmd.kill_on_drop(true)
        .arg(env::var("PERF_CALC").unwrap())
        .arg("difficulty")
        .arg(map_path);
    if !mods.is_empty() {
        for m in mods.iter().filter(|&m| m != GameMods::ScoreV2) {
            cmd.arg("-m").arg(m.to_string());
        }
    }
    let output = match time::timeout(time::Duration::from_secs(10), cmd.output()).await {
        Ok(output) => output?,
        Err(_) => bail!("Timeout while waiting for stars output"),
    };
    if output.status.success() {
        let result = String::from_utf8(output.stdout)
            .map_err(|_| format_err!("Could not read stdout string"))?;
        f32::from_str(&result.trim())
            .map_err(|_| format_err!("PerfCalc result could not be parsed into stars"))
    } else {
        let error_msg = String::from_utf8(output.stderr)
            .map_err(|_| format_err!("Could not read stderr string"))?;
        bail!(error_msg)
    }
}

// Calculate CTB pp manually in case its useful at some point...
fn _ctb_score_pp(score: &Score, map: &Beatmap, stars: f64) -> f32 {
    let mods = &score.enabled_mods;
    let fruits_hit = score.count300;
    let ticks_hit = score.count100;
    let misses = score.count_miss;

    // let stars = if mods.contains(&GameMod::Easy)
    //     || mods.contains(&GameMod::HardRock)
    //     || mods.contains(&GameMod::DoubleTime)
    //     || mods.contains(&GameMod::NightCore)
    //     || mods.contains(&GameMod::HalfTime)
    // {
    //     todo!()
    // } else {
    //     map.stars as f64
    // };

    let num_total_hits = (misses + ticks_hit + fruits_hit) as f64;

    let mut value = (5.0_f64 * 1.0_f64.max(stars / 0.0049) - 4.0).powi(2) / 100_000.0;

    // Longer maps are worth more. "Longer" means how many hits there are which can contribute to combo
    let mut length_bonus = 0.95_f64 + 0.3 * (num_total_hits / 2500.0).min(1.0);
    if num_total_hits > 2_500.0 {
        length_bonus += (num_total_hits / 2_500.0).log10() * 0.475;
    }
    value *= length_bonus;

    // Penalize misses exponentially. This mainly fixes tag4 maps and the likes until a per-hitobject solution is available
    value *= 0.97_f64.powi(misses as i32);

    // Combo scaling
    match map.max_combo {
        Some(max_combo) if max_combo > 0 => {
            value *= (score.max_combo as f64 / max_combo as f64)
                .powf(0.8)
                .min(1.0)
        }
        _ => {}
    }

    let mut ar_factor = 1.0_f64;
    if map.diff_ar > 9.0 {
        ar_factor += 0.1 * (map.diff_ar as f64 - 9.0); // 10% for each AR above 9
    }
    if map.diff_ar > 10.0 {
        ar_factor += 0.1 * (map.diff_ar as f64 - 10.0); // Additional 10% at AR 11, 30% total
    } else if map.diff_ar < 8.0 {
        ar_factor += 0.025 * (8.0 - map.diff_ar as f64); // 2.5% for each AR below 8
    }
    value *= ar_factor;

    if mods.contains(GameMods::Hidden) {
        value *= 1.05 + 0.075 * (10.0 - map.diff_ar.min(10.0) as f64); // 7.5% for each AR below 10

        // Hiddens gives almost nothing on max approach rate, and more the lower it is
        if map.diff_ar <= 10.0 {
            value *= 1.05 + 0.075 * (10.0 - map.diff_ar as f64); // 7.5% for each AR below 10
        } else if map.diff_ar > 10.0 {
            value *= 1.01 + 0.04 * (11.0 - map.diff_ar.min(11.0) as f64); // 5% at AR 10, 1% at AR 11
        }
    }

    // Apply length bonus again if flashlight is on simply because it becomes a lot harder on longer maps.
    if mods.contains(GameMods::Flashlight) {
        value *= 1.35 * length_bonus;
    }

    // Scale the aim value with accuracy _slightly_
    value *= (score.accuracy(GameMode::CTB) as f64).powf(5.5);

    // Custom multipliers for NoFail. SpunOut is not applicable.
    if mods.contains(GameMods::NoFail) {
        value *= 0.9;
    }

    value as f32
}

fn f32_to_db(
    in_db: bool,
    mysql: &MySQL,
    map_id: u32,
    mode: GameMode,
    mods: GameMods,
    value: f32,
    stars: bool, // max_pp if false
) -> Result<(), Error> {
    if in_db {
        if stars {
            match mysql.update_stars_map(map_id, mode, mods, value) {
                Ok(_) => debug!(
                    "Updated map id {} with mods {} in {} stars table",
                    map_id, mods, mode
                ),
                Err(why) => {
                    error!("Error while updating {} stars: {}", mode, why);
                    return Err(why);
                }
            }
        } else {
            match mysql.update_pp_map(map_id, mode, mods, value) {
                Ok(_) => debug!(
                    "Updated map id {} with mods {} in {} pp table",
                    map_id, mods, mode
                ),
                Err(why) => {
                    error!("Error while updating {} pp: {}", mode, why);
                    return Err(why);
                }
            }
        }
    } else if stars {
        match mysql.insert_stars_map(map_id, mode, mods, value) {
            Ok(_) => debug!("Inserted beatmap {} into {} stars table", map_id, mode),
            Err(why) => {
                error!("Error while inserting {} stars: {}", mode, why);
                return Err(why);
            }
        }
    } else {
        match mysql.insert_pp_map(map_id, mode, mods, value) {
            Ok(_) => debug!("Inserted beatmap {} into {} pp table", map_id, mode),
            Err(why) => {
                error!("Error while inserting {} pp: {}", mode, why);
                return Err(why);
            }
        }
    }
    Ok(())
}

enum CalcParam<'a, S: SubScore> {
    #[allow(dead_code)] // its not dead code rust...
    MNA { score: Option<u32>, mods: GameMods },
    #[allow(dead_code)]
    CTB { score: &'a S },
    #[allow(dead_code)]
    MaxCTB { mods: GameMods },
}

impl<'a, S: SubScore> CalcParam<'a, S> {
    fn mania(score: Option<u32>, mods: GameMods) -> Self {
        Self::MNA { score, mods }
    }

    fn ctb(score: &'a S) -> Self
    where
        S: SubScore,
    {
        Self::CTB { score }
    }

    fn max_ctb(mods: GameMods) -> Self {
        Self::MaxCTB { mods }
    }

    fn mode(&self) -> GameMode {
        match self {
            CalcParam::MNA { .. } => GameMode::MNA,
            CalcParam::CTB { .. } | CalcParam::MaxCTB { .. } => GameMode::CTB,
        }
    }

    fn mods(&self) -> GameMods {
        match self {
            CalcParam::MNA { mods, .. } | CalcParam::MaxCTB { mods } => *mods,
            CalcParam::CTB { score, .. } => score.mods(),
        }
    }
}

pub trait SubScore {
    fn miss(&self) -> u32;
    fn c50(&self) -> u32;
    fn c100(&self) -> u32;
    fn c300(&self) -> u32;
    fn combo(&self) -> u32;
    fn mods(&self) -> GameMods;
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
}

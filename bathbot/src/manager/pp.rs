use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
};

use bathbot_model::{OsuStatsScore, ScoreSlim};
use bathbot_psql::Database;
use eyre::Result;
use rosu_pp::{
    Beatmap, BeatmapExt, DifficultyAttributes, GameMode as Mode, PerformanceAttributes, ScoreState,
};
use rosu_v2::{
    model::mods::GameMods,
    prelude::{GameMode, Grade, Score},
};

use super::OsuMap;
use crate::commands::osu::LeaderboardScore;

#[derive(Clone)]
pub struct PpManager<'d, 'm> {
    psql: &'d Database,
    map: Cow<'m, Beatmap>,
    map_id: u32,
    is_convert: bool,
    attrs: Option<DifficultyAttributes>,
    mode: GameMode,
    mods: Mods,
    state: Option<ScoreState>,
    partial: bool,
}

impl<'d, 'm> PpManager<'d, 'm> {
    pub fn new(map: &'m OsuMap, psql: &'d Database) -> Self {
        Self::from_parsed(&map.pp_map, map.map_id(), map.mode(), map.is_convert, psql)
    }

    pub fn from_parsed(
        map: &'m Beatmap,
        map_id: u32,
        mode: GameMode,
        is_convert: bool,
        psql: &'d Database,
    ) -> Self {
        Self {
            psql,
            map: Cow::Borrowed(map),
            map_id,
            is_convert,
            attrs: None,
            mode,
            mods: Mods::default(),
            state: None,
            partial: false,
        }
    }

    /// Use the given attributes. Be sure they match they match the map and
    /// mods!
    pub fn attributes(&mut self, attrs: DifficultyAttributes) {
        self.attrs = Some(attrs);
    }

    pub fn mode(mut self, mode: GameMode) -> Self {
        if self.mode != mode {
            self.attrs = None;
        }

        if let Cow::Owned(map) = self.map.convert_mode(Self::mode_conversion(mode)) {
            self.map = Cow::Owned(map);
            self.is_convert = true;
        } else if mode == GameMode::Catch && self.mode != GameMode::Catch {
            self.is_convert = true;
        }

        self.mode = mode;

        self
    }

    pub fn mods(self, mods: impl Into<Mods>) -> Self {
        fn inner<'d, 'm>(mut this: PpManager<'d, 'm>, mods: Mods) -> PpManager<'d, 'm> {
            if this.mods != mods {
                this.attrs = None;
            }

            this.mods = mods;

            this
        }

        inner(self, mods.into())
    }

    pub fn score(mut self, score: impl Into<ScoreData>) -> Self {
        let ScoreData {
            state,
            mods,
            mode,
            partial,
        } = score.into();

        self.state = Some(state);
        self.partial = partial;

        if let Some(mode) = mode {
            self = self.mode(mode);
        }

        self.mods(mods)
    }

    async fn lookup_attrs(&self) -> Result<Option<DifficultyAttributes>> {
        if self.mods.clock_rate.is_some() {
            return Ok(None);
        }

        self.psql
            .select_map_difficulty_attrs(self.map_id, self.mode, self.mods.bits)
            .await
    }

    /// Calculate difficulty attributes
    pub async fn difficulty(&mut self) -> &DifficultyAttributes {
        if !self.partial {
            match self.attrs {
                Some(ref attrs) => return attrs,
                None => match self.lookup_attrs().await {
                    Ok(Some(attrs)) => return self.attrs.insert(attrs),
                    Ok(None) => {}
                    Err(err) => warn!(?err, "Failed to get difficulty attributes"),
                },
            }
        }

        let mode = Self::mode_conversion(self.mode);

        let mut calc = self
            .map
            .stars()
            .mode(mode)
            .mods(self.mods.bits)
            .is_convert(self.is_convert);

        if let Some(clock_rate) = self.mods.clock_rate {
            calc = calc.clock_rate(f64::from(clock_rate));
        }

        if let Some(state) = self.state.as_ref().filter(|_| self.partial) {
            calc = calc.passed_objects(state.total_hits(mode));
        }

        let attrs = calc.calculate();

        if !self.partial && self.mods.clock_rate.is_some() {
            let upsert_fut = self
                .psql
                .upsert_map_difficulty(self.map_id, self.mods.bits, &attrs);

            if let Err(err) = upsert_fut.await {
                warn!(?err, "Failed to upsert difficulty attrs");
            }
        }

        self.attrs.insert(attrs)
    }

    /// Calculate performance attributes
    pub async fn performance(&mut self) -> PerformanceAttributes {
        let attrs = self.difficulty().await.to_owned();
        let mode = Self::mode_conversion(self.mode);

        let mut calc = self
            .map
            .pp()
            .attributes(attrs)
            .mode(mode)
            .mods(self.mods.bits)
            .is_convert(self.is_convert);

        if let Some(clock_rate) = self.mods.clock_rate {
            calc = calc.clock_rate(f64::from(clock_rate));
        }

        if let Some(state) = self.state.take() {
            if self.partial {
                calc = calc.passed_objects(state.total_hits(mode));
            }

            calc = calc.state(state);
        }

        calc.calculate()
    }

    /// Convert from `rosu_v2` mode to `rosu_pp` mode
    pub fn mode_conversion(mode: GameMode) -> Mode {
        Mode::from(mode as u8)
    }
}

pub struct ScoreData {
    state: ScoreState,
    mods: Mods,
    mode: Option<GameMode>,
    partial: bool,
}

impl<'s> From<&'s Score> for ScoreData {
    #[inline]
    fn from(score: &'s Score) -> Self {
        let stats = score.statistics.as_legacy(score.mode);

        Self {
            state: ScoreState {
                max_combo: score.max_combo as usize,
                n_geki: stats.count_geki as usize,
                n_katu: stats.count_katu as usize,
                n300: stats.count_300 as usize,
                n100: stats.count_100 as usize,
                n50: stats.count_50 as usize,
                n_misses: stats.count_miss as usize,
            },
            mods: Mods::from(&score.mods),
            mode: Some(score.mode),
            partial: !score.passed,
        }
    }
}

impl<'s> From<&'s ScoreSlim> for ScoreData {
    #[inline]
    fn from(score: &'s ScoreSlim) -> Self {
        Self {
            state: ScoreState {
                max_combo: score.max_combo as usize,
                n_geki: score.statistics.count_geki as usize,
                n_katu: score.statistics.count_katu as usize,
                n300: score.statistics.count_300 as usize,
                n100: score.statistics.count_100 as usize,
                n50: score.statistics.count_50 as usize,
                n_misses: score.statistics.count_miss as usize,
            },
            mods: Mods::from(&score.mods),
            mode: Some(score.mode),
            partial: score.grade == Grade::F,
        }
    }
}

impl<'s> From<&'s OsuStatsScore> for ScoreData {
    #[inline]
    fn from(score: &'s OsuStatsScore) -> Self {
        Self {
            state: ScoreState {
                max_combo: score.max_combo as usize,
                n_geki: score.count_geki as usize,
                n_katu: score.count_katu as usize,
                n300: score.count300 as usize,
                n100: score.count100 as usize,
                n50: score.count50 as usize,
                n_misses: score.count_miss as usize,
            },
            mods: Mods::from(&score.mods),
            mode: None,
            partial: score.grade == Grade::F,
        }
    }
}

impl<'s> From<&'s LeaderboardScore> for ScoreData {
    fn from(score: &'s LeaderboardScore) -> Self {
        Self {
            state: ScoreState {
                max_combo: score.combo as usize,
                n_geki: score.statistics.count_geki as usize,
                n_katu: score.statistics.count_katu as usize,
                n300: score.statistics.count_300 as usize,
                n100: score.statistics.count_100 as usize,
                n50: score.statistics.count_50 as usize,
                n_misses: score.statistics.count_miss as usize,
            },
            mods: Mods::from(&score.mods),
            mode: Some(score.mode),
            partial: score.grade == Grade::F,
        }
    }
}

/// Mods with an optional custom clock rate.
#[derive(Copy, Clone, Default, PartialEq)]
pub struct Mods {
    pub bits: u32,
    pub clock_rate: Option<f32>,
}

impl Mods {
    /// Create new [`Mods`] without a custom clock rate.
    pub fn new(bits: u32) -> Self {
        Self {
            bits,
            clock_rate: None,
        }
    }
}

impl From<&GameMods> for Mods {
    fn from(mods: &GameMods) -> Self {
        Self {
            bits: mods.bits(),
            clock_rate: mods.clock_rate(),
        }
    }
}

// Little iffy due to the contained f32 but required to be usable as HashMap key
impl Eq for Mods {}

impl Hash for Mods {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bits.hash(state);
        self.clock_rate.map(f32::to_bits).hash(state);
    }
}

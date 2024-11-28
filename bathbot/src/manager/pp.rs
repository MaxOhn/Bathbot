use std::borrow::Cow;

use bathbot_model::{OsuStatsScore, ScoreSlim};
use eyre::Result;
use rosu_pp::{
    any::{DifficultyAttributes, PerformanceAttributes, ScoreState},
    model::mode::GameMode as Mode,
    Beatmap, Difficulty,
};
use rosu_v2::{
    model::mods::GameMods,
    prelude::{GameMode, Grade, Score, ScoreStatistics},
};

use super::OsuMap;
use crate::commands::{osu::LeaderboardScore, utility::ScoreEmbedDataRaw};

#[derive(Clone)]
pub struct PpManager<'m> {
    map: Cow<'m, Beatmap>,
    attrs: Option<DifficultyAttributes>,
    mods: Mods,
    state: Option<ScoreState>,
    partial: bool,
    lazer: bool,
}

impl<'m> PpManager<'m> {
    pub fn new(map: &'m OsuMap) -> Self {
        Self::from_parsed(&map.pp_map)
    }

    pub fn from_parsed(map: &'m Beatmap) -> Self {
        Self {
            map: Cow::Borrowed(map),
            attrs: None,
            mods: Mods::default(),
            state: None,
            partial: false,
            lazer: true,
        }
    }

    pub fn lazer(mut self, lazer: bool) -> Self {
        self.lazer = lazer;

        self
    }

    pub fn mode(mut self, mode: GameMode) -> Self {
        let map = match self.map {
            Cow::Borrowed(map) => match (map.mode, mode) {
                (Mode::Osu, GameMode::Taiko) => self.map.to_mut(),
                (Mode::Osu, GameMode::Catch) => self.map.to_mut(),
                (Mode::Osu, GameMode::Mania) => self.map.to_mut(),
                _ => return self,
            },
            Cow::Owned(ref mut map) => map,
        };

        let mode = (mode as u8).into();

        if map.mode != mode {
            self.attrs = None;
        }

        let _ = map.convert_mut(mode, &self.mods.inner);

        self
    }

    pub fn mods(self, mods: impl Into<Mods>) -> Self {
        fn inner(mut this: PpManager<'_>, mods: Mods) -> PpManager<'_> {
            if this.mods != mods {
                this.attrs = None;
            }

            this.mods = mods;

            this
        }

        inner(self, mods.into())
    }

    pub fn score(self, score: impl Into<ScoreData>) -> Self {
        fn inner(mut manager: PpManager<'_>, score: ScoreData) -> PpManager<'_> {
            let ScoreData {
                state,
                mods,
                mode,
                partial,
                lazer,
            } = score;

            manager.state = Some(state);
            manager.partial = partial;
            manager.lazer = lazer;

            if let Some(mode) = mode {
                manager = manager.mode(mode);
            }

            manager.mods(mods)
        }

        inner(self, score.into())
    }

    async fn lookup_attrs(&self) -> Result<Option<DifficultyAttributes>> {
        // TODO: consider custom mod parameters
        Ok(None)

        // if self.mods.clock_rate.is_some() {
        //     return Ok(None);
        // }

        // let mode = GameMode::from(self.map.mode as u8);

        // Context::psql()
        //     .select_map_difficulty_attrs(self.map_id, mode, self.mods.bits)
        //     .await
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

        let mut calc = Difficulty::new()
            .mods(self.mods.inner.clone())
            .lazer(self.lazer);

        if let Some(clock_rate) = self.mods.clock_rate {
            calc = calc.clock_rate(clock_rate);
        }

        if let Some(state) = self.state.as_ref().filter(|_| self.partial) {
            calc = calc.passed_objects(state.total_hits(self.map.mode));
        }

        let attrs = calc.calculate(&self.map);

        // TODO: store calculated attributes
        // if !self.partial && self.mods.clock_rate.is_none() {
        //     let upsert_fut =
        //         Context::psql().upsert_map_difficulty(self.map_id, self.mods.bits,
        // &attrs);

        //     if let Err(err) = upsert_fut.await {
        //         warn!(?err, "Failed to upsert difficulty attrs");
        //     }
        // }

        self.attrs.insert(attrs)
    }

    /// Calculate performance attributes
    pub async fn performance(&mut self) -> PerformanceAttributes {
        let mut calc = self
            .difficulty()
            .await
            .to_owned()
            .performance()
            .mods(self.mods.inner.clone())
            .lazer(self.lazer);

        if let Some(clock_rate) = self.mods.clock_rate {
            calc = calc.clock_rate(clock_rate);
        }

        if let Some(state) = self.state.take() {
            if self.partial {
                calc = calc.passed_objects(state.total_hits(self.map.mode));
            }

            calc = calc.state(state);
        }

        calc.calculate()
    }
}

pub struct ScoreData {
    state: ScoreState,
    mods: Mods,
    mode: Option<GameMode>,
    partial: bool,
    lazer: bool,
}

impl<'s> From<&'s Score> for ScoreData {
    #[inline]
    fn from(score: &'s Score) -> Self {
        Self {
            state: stats_to_state(score.max_combo, score.mode, &score.statistics),
            mods: Mods::new(score.mods.clone()),
            mode: Some(score.mode),
            partial: !score.passed,
            lazer: score.set_on_lazer,
        }
    }
}

impl<'s> From<&'s ScoreSlim> for ScoreData {
    #[inline]
    fn from(score: &'s ScoreSlim) -> Self {
        Self {
            state: stats_to_state(score.max_combo, score.mode, &score.statistics),
            mods: Mods::new(score.mods.clone()),
            mode: Some(score.mode),
            partial: score.grade == Grade::F,
            lazer: score.set_on_lazer,
        }
    }
}

impl<'s> From<&'s OsuStatsScore> for ScoreData {
    #[inline]
    fn from(score: &'s OsuStatsScore) -> Self {
        Self {
            state: ScoreState {
                max_combo: score.max_combo,
                n_geki: score.count_geki,
                n_katu: score.count_katu,
                n300: score.count300,
                n100: score.count100,
                n50: score.count50,
                misses: score.count_miss,
                osu_large_tick_hits: 0,
                slider_end_hits: 0,
            },
            mods: Mods::new(score.mods.clone()),
            mode: None,
            partial: score.grade == Grade::F,
            lazer: false,
        }
    }
}

impl<'s> From<&'s LeaderboardScore> for ScoreData {
    fn from(score: &'s LeaderboardScore) -> Self {
        Self {
            state: stats_to_state(score.combo, score.mode, &score.statistics),
            mods: Mods::new(score.mods.clone()),
            mode: Some(score.mode),
            partial: score.grade == Grade::F,
            lazer: !score.is_legacy,
        }
    }
}

impl<'s> From<&'s ScoreEmbedDataRaw> for ScoreData {
    #[inline]
    fn from(score: &'s ScoreEmbedDataRaw) -> Self {
        Self {
            state: stats_to_state(score.max_combo, score.mode, &score.statistics),
            mods: Mods::new(score.mods.clone()),
            mode: Some(score.mode),
            partial: score.grade == Grade::F,
            lazer: !score.legacy_scores,
        }
    }
}

fn stats_to_state(max_combo: u32, mode: GameMode, stats: &ScoreStatistics) -> ScoreState {
    let n_geki = match mode {
        GameMode::Osu | GameMode::Taiko | GameMode::Catch => 0,
        GameMode::Mania => stats.good,
    };

    let n_katu = match mode {
        GameMode::Osu | GameMode::Taiko => 0,
        GameMode::Catch => stats.small_tick_miss.max(stats.good),
        GameMode::Mania => stats.good,
    };

    let n100 = match mode {
        GameMode::Osu | GameMode::Taiko | GameMode::Mania => stats.ok,
        GameMode::Catch => stats.large_tick_hit.max(stats.ok),
    };

    let n50 = match mode {
        GameMode::Osu | GameMode::Mania => stats.meh,
        GameMode::Taiko => 0,
        GameMode::Catch => stats.small_tick_hit.max(stats.meh),
    };

    let osu_large_tick_hits = match mode {
        GameMode::Osu => stats.large_tick_hit,
        GameMode::Taiko | GameMode::Catch | GameMode::Mania => 0,
    };

    let slider_end_hits = match mode {
        GameMode::Osu => {
            if stats.slider_tail_hit > 0 {
                stats.slider_tail_hit
            } else {
                stats.small_tick_hit
            }
        }
        GameMode::Taiko | GameMode::Catch | GameMode::Mania => 0,
    };

    ScoreState {
        max_combo,
        osu_large_tick_hits,
        slider_end_hits,
        n_geki,
        n_katu,
        n300: stats.great,
        n100,
        n50,
        misses: stats.miss,
    }
}

/// Mods with an optional custom clock rate.
#[derive(Clone, Default, PartialEq)]
pub struct Mods {
    pub inner: rosu_pp::GameMods,
    pub clock_rate: Option<f64>,
}

impl Mods {
    /// Create new [`Mods`] without a custom clock rate.
    pub fn new(mods: impl Into<rosu_pp::GameMods>) -> Self {
        Self {
            inner: mods.into(),
            clock_rate: None,
        }
    }
}

impl From<GameMods> for Mods {
    fn from(mods: GameMods) -> Self {
        Self::new(mods)
    }
}

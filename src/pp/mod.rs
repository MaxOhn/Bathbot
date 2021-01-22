use crate::{
    util::{error::PPError, osu::prepare_beatmap_file, BeatmapExt, ScoreExt},
    BotResult,
};

use bitflags::bitflags;
use rosu::model::{GameMode, GameMods, Grade};
use rosu_pp::{
    Beatmap, BeatmapExt as RosuBeatmapExt, FruitsPP, GameMode as Mode, ManiaPP, OsuPP, TaikoPP,
};
use std::fs::File;

bitflags! {
    pub struct Calculations: u8 {
        const PP = 1;
        const MAX_PP = 2;
        const STARS = 4;
    }
}

pub struct PPCalculator<'s, 'm> {
    score: Option<Box<dyn ScoreExt + 's>>,
    map: Option<Box<dyn BeatmapExt + 'm>>,

    mods: Option<GameMods>,

    pp: Option<f32>,
    max_pp: Option<f32>,
    stars: Option<f32>,
}

impl<'s, 'm> PPCalculator<'s, 'm> {
    #[inline]
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

    #[inline]
    pub fn mods(mut self, mods: GameMods) -> Self {
        self.mods.replace(mods);

        self
    }

    #[inline]
    pub fn score(mut self, score: impl ScoreExt + 's) -> Self {
        self.score.replace(Box::new(score));

        self
    }

    #[inline]
    pub fn map(mut self, map: impl BeatmapExt + 'm) -> Self {
        self.map.replace(Box::new(map));

        self
    }

    pub async fn calculate(&mut self, calcs: Calculations) -> BotResult<()> {
        assert_ne!(calcs.bits, 0);

        let map = match self.map.as_deref() {
            Some(map) => {
                let map_path = prepare_beatmap_file(map.map_id()).await?;
                let file = File::open(map_path).map_err(PPError::IoError)?;
                Beatmap::parse(file).map_err(PPError::from)?
            }
            None => return Err(PPError::NoMapId.into()),
        };

        let score = self.score.as_deref();
        let mods = score
            .map_or_else(|| self.mods.unwrap_or(GameMods::NoMod), |s| s.mods())
            .bits();

        // Max PP
        let max_pp_result = if calcs.contains(Calculations::MAX_PP) {
            Some(map.max_pp(mods))
        } else {
            None
        };

        let max_pp = max_pp_result.as_ref().map(|result| result.pp());
        let mut stars = max_pp_result.as_ref().map(|result| result.stars());

        // Score PP
        let pp_result = if calcs.contains(Calculations::PP) {
            let result = match map.mode {
                Mode::STD => {
                    let (misses, n300, n100, n50, combo, hits) = match score {
                        Some(score) => (
                            score.count_miss() as usize,
                            score.count_300() as usize,
                            score.count_100() as usize,
                            score.count_50() as usize,
                            Some(score.max_combo()),
                            Some(score.hits(map.mode as u8)),
                        ),
                        None => (0, 0, 0, 0, None, None),
                    };

                    let mut calculator = OsuPP::new(&map)
                        .mods(mods)
                        .misses(misses)
                        .n300(n300)
                        .n100(n100)
                        .n50(n50);

                    if let Some(combo) = combo {
                        calculator = calculator.combo(combo as usize);
                    }

                    if let Some(hits) = hits {
                        calculator = calculator.passed_objects(hits as usize);
                    }

                    // Reuse attributes only if the play is not a fail
                    if let Some(result) = max_pp_result
                        .filter(|_| score.map_or(true, |s| s.grade(GameMode::STD) != Grade::F))
                    {
                        calculator.attributes(result).calculate()
                    } else {
                        calculator.calculate()
                    }
                }
                Mode::MNA => {
                    let score = score.map_or(1_000_000, |s| s.score());

                    let calculator = ManiaPP::new(&map).mods(mods).score(score);

                    if let Some(result) = max_pp_result {
                        calculator.attributes(result).calculate()
                    } else {
                        calculator.calculate()
                    }
                }
                Mode::CTB => {
                    let (acc, combo, misses, hits) = match score {
                        Some(score) => (
                            score.acc(GameMode::CTB),
                            Some(score.max_combo()),
                            score.count_miss() as usize,
                            Some(score.hits(GameMode::CTB as u8)),
                        ),
                        None => (100.0, None, 0, None),
                    };

                    let mut calculator = FruitsPP::new(&map).mods(mods).misses(misses);

                    // Reuse attributes only if the play is not a fail
                    if let Some(result) = max_pp_result
                        .filter(|_| score.map_or(true, |s| s.grade(GameMode::TKO) != Grade::F))
                    {
                        calculator = calculator.attributes(result);
                    }

                    if let Some(combo) = combo {
                        calculator = calculator.combo(combo as usize);
                    }

                    if let Some(hits) = hits {
                        calculator = calculator.passed_objects(hits as usize);
                    }

                    calculator.accuracy(acc).calculate()
                }
                Mode::TKO => {
                    let (misses, acc, combo, hits) = match score {
                        Some(score) => (
                            score.count_miss() as usize,
                            score.acc(GameMode::TKO),
                            Some(score.max_combo()),
                            Some(score.hits(map.mode as u8)),
                        ),
                        None => (0, 100.0, None, None),
                    };

                    let mut calculator = TaikoPP::new(&map).mods(mods).misses(misses).accuracy(acc);

                    if let Some(combo) = combo {
                        calculator = calculator.combo(combo as usize);
                    }

                    if let Some(hits) = hits {
                        calculator = calculator.passed_objects(hits as usize);
                    }

                    // Reuse attributes only if the play is not a fail
                    if let Some(result) = max_pp_result
                        .filter(|_| score.map_or(true, |s| s.grade(GameMode::TKO) != Grade::F))
                    {
                        calculator.attributes(result).calculate()
                    } else {
                        calculator.calculate()
                    }
                }
            };

            Some(result)
        } else {
            None
        };

        let pp = pp_result.as_ref().map(|result| result.pp());
        stars = stars.or_else(|| {
            pp_result
                .filter(|_| score.map_or(true, |s| s.grade(GameMode::TKO) != Grade::F))
                .map(|result| result.stars())
        });

        // Stars
        if stars.is_none() && calcs.contains(Calculations::STARS) {
            stars = Some(map.stars(mods, None).stars());
        }

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

    #[inline]
    pub fn pp(&self) -> Option<f32> {
        self.pp
    }

    #[inline]
    pub fn max_pp(&self) -> Option<f32> {
        self.max_pp
    }

    #[inline]
    pub fn stars(&self) -> Option<f32> {
        self.stars
    }
}

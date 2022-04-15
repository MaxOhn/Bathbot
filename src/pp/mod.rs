use rosu_pp::{Beatmap, BeatmapExt as rosu_v2BeatmapExt, DifficultyAttributes, ScoreState};
use rosu_v2::model::GameMods;

use crate::{
    core::Context,
    error::PpError,
    util::{osu::prepare_beatmap_file, ScoreExt},
};

enum ScoreKind<'s> {
    Mods(GameMods),
    Score(&'s dyn ScoreExt),
}

impl ScoreKind<'_> {
    fn mods(&self) -> u32 {
        match self {
            Self::Mods(mods) => mods.bits(),
            Self::Score(score) => score.mods().bits(),
        }
    }

    fn state(&self) -> ScoreState {
        match self {
            Self::Mods(_) => ScoreState::default(),
            Self::Score(score) => ScoreState {
                max_combo: score.max_combo() as usize,
                n_katu: score.count_katu() as usize,
                n300: score.count_300() as usize,
                n100: score.count_100() as usize,
                n50: score.count_50() as usize,
                misses: score.count_miss() as usize,
                score: score.score(),
            },
        }
    }
}

pub struct PpCalculator<'s> {
    map: Beatmap,
    score: Option<ScoreKind<'s>>,
    difficulty: Option<DifficultyAttributes>,
}

impl<'s> PpCalculator<'s> {
    pub async fn new(ctx: &Context, map_id: u32) -> Result<PpCalculator<'s>, PpError> {
        let map_path = prepare_beatmap_file(ctx, map_id).await?;
        let map = Beatmap::from_path(map_path).await?;

        Ok(Self {
            map,
            score: None,
            difficulty: None,
        })
    }

    pub fn mods(&mut self, mods: GameMods) -> &mut Self {
        self.score = Some(ScoreKind::Mods(mods));
        self.difficulty = None;

        self
    }

    pub fn score(&mut self, score: &'s dyn ScoreExt) -> &mut Self {
        self.score = Some(ScoreKind::Score(score));
        self.difficulty = None;

        self
    }

    pub fn stars(&mut self) -> f64 {
        let mods = self.score.as_ref().map(ScoreKind::mods).unwrap_or_default();

        let difficulty = &mut self.difficulty;
        let map = &self.map;

        difficulty
            .get_or_insert_with(|| map.stars().mods(mods).calculate())
            .stars()
    }

    pub fn max_pp(&mut self) -> f64 {
        let mods = self.score.as_ref().map(ScoreKind::mods).unwrap_or_default();

        let difficulty = &mut self.difficulty;
        let map = &self.map;

        let difficulty = difficulty
            .get_or_insert_with(|| map.stars().mods(mods).calculate())
            .to_owned();

        map.pp().attributes(difficulty).mods(mods).calculate().pp()
    }

    pub fn pp(&mut self) -> f64 {
        let mods = self.score.as_ref().map(ScoreKind::mods).unwrap_or_default();

        let state = self
            .score
            .as_ref()
            .map(ScoreKind::state)
            .unwrap_or_default();

        let difficulty = &mut self.difficulty;
        let map = &self.map;

        let difficulty = difficulty
            .get_or_insert_with(|| map.stars().mods(mods).calculate())
            .to_owned();

        map.pp()
            .attributes(difficulty)
            .state(state)
            .mods(mods)
            .calculate()
            .pp()
    }
}

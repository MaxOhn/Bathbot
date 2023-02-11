use bathbot_model::ScoreSlim;
use rosu_pp::ScoreState;
use rosu_v2::prelude::Score;

pub trait ScoreHasState {
    fn max_combo(&self) -> u32;
    fn n_miss(&self) -> u32;
    fn n_geki(&self) -> u32;
    fn n300(&self) -> u32;
    fn n_katu(&self) -> u32;
    fn n100(&self) -> u32;
    fn n50(&self) -> u32;

    #[inline]
    fn state(&self) -> ScoreState {
        ScoreState {
            max_combo: self.max_combo() as usize,
            n_misses: self.n_miss() as usize,
            n_geki: self.n_geki() as usize,
            n300: self.n300() as usize,
            n_katu: self.n_katu() as usize,
            n100: self.n100() as usize,
            n50: self.n50() as usize,
        }
    }
}

#[rustfmt::skip]
impl ScoreHasState for Score {
    fn max_combo(&self) -> u32 { self.max_combo }
    fn n_miss(&self) -> u32 { self.statistics.count_miss }
    fn n_geki(&self) -> u32 { self.statistics.count_geki }
    fn n300(&self) -> u32 { self.statistics.count_300 }
    fn n_katu(&self) -> u32 { self.statistics.count_katu }
    fn n100(&self) -> u32 { self.statistics.count_100 }
    fn n50(&self) -> u32 { self.statistics.count_50 }
}

#[rustfmt::skip]
impl ScoreHasState for ScoreSlim {
    fn max_combo(&self) -> u32 { self.max_combo }
    fn n_miss(&self) -> u32 { self.statistics.count_miss }
    fn n_geki(&self) -> u32 { self.statistics.count_geki }
    fn n300(&self) -> u32 { self.statistics.count_300 }
    fn n_katu(&self) -> u32 { self.statistics.count_katu }
    fn n100(&self) -> u32 { self.statistics.count_100 }
    fn n50(&self) -> u32 { self.statistics.count_50 }
}

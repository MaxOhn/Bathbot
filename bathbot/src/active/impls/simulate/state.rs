use rosu_pp::{
    catch::CatchScoreState, mania::ManiaScoreState, osu::OsuScoreState, taiko::TaikoScoreState,
};
use rosu_v2::prelude::{GameMode, ScoreStatistics};

pub(super) enum ScoreState {
    Osu(OsuScoreState),
    Taiko(TaikoScoreState),
    Catch(CatchScoreState),
    Mania(ManiaScoreState),
}

impl ScoreState {
    pub(super) fn into_parts(self) -> (GameMode, ScoreStatistics) {
        match self {
            Self::Osu(state) => {
                let stats = ScoreStatistics {
                    count_geki: 0,
                    count_300: state.n300 as u32,
                    count_katu: 0,
                    count_100: state.n100 as u32,
                    count_50: state.n50 as u32,
                    count_miss: state.n_misses as u32,
                };

                (GameMode::Osu, stats)
            }
            Self::Taiko(state) => {
                let stats = ScoreStatistics {
                    count_geki: 0,
                    count_300: state.n300 as u32,
                    count_katu: 0,
                    count_100: state.n100 as u32,
                    count_50: 0,
                    count_miss: state.n_misses as u32,
                };

                (GameMode::Taiko, stats)
            }
            Self::Catch(state) => {
                let stats = ScoreStatistics {
                    count_geki: 0,
                    count_300: state.n_fruits as u32,
                    count_katu: state.n_tiny_droplet_misses as u32,
                    count_100: state.n_droplets as u32,
                    count_50: state.n_tiny_droplets as u32,
                    count_miss: state.n_misses as u32,
                };

                (GameMode::Catch, stats)
            }
            Self::Mania(state) => {
                let stats = ScoreStatistics {
                    count_geki: state.n320 as u32,
                    count_300: state.n300 as u32,
                    count_katu: state.n200 as u32,
                    count_100: state.n100 as u32,
                    count_50: state.n50 as u32,
                    count_miss: state.n_misses as u32,
                };

                (GameMode::Mania, stats)
            }
        }
    }
}

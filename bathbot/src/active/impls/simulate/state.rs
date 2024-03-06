use rosu_pp::{
    catch::CatchScoreState, mania::ManiaScoreState, osu::OsuScoreState, taiko::TaikoScoreState,
};
use rosu_v2::{model::score::LegacyScoreStatistics, prelude::GameMode};

pub(super) enum ScoreState {
    Osu(OsuScoreState),
    Taiko(TaikoScoreState),
    Catch(CatchScoreState),
    Mania(ManiaScoreState),
}

impl ScoreState {
    pub(super) fn into_parts(self) -> (GameMode, LegacyScoreStatistics) {
        match self {
            Self::Osu(state) => {
                let stats = LegacyScoreStatistics {
                    count_geki: 0,
                    count_300: state.n300,
                    count_katu: 0,
                    count_100: state.n100,
                    count_50: state.n50,
                    count_miss: state.misses,
                };

                (GameMode::Osu, stats)
            }
            Self::Taiko(state) => {
                let stats = LegacyScoreStatistics {
                    count_geki: 0,
                    count_300: state.n300,
                    count_katu: 0,
                    count_100: state.n100,
                    count_50: 0,
                    count_miss: state.misses,
                };

                (GameMode::Taiko, stats)
            }
            Self::Catch(state) => {
                let stats = LegacyScoreStatistics {
                    count_geki: 0,
                    count_300: state.fruits,
                    count_katu: state.tiny_droplet_misses,
                    count_100: state.droplets,
                    count_50: state.tiny_droplets,
                    count_miss: state.misses,
                };

                (GameMode::Catch, stats)
            }
            Self::Mania(state) => {
                let stats = LegacyScoreStatistics {
                    count_geki: state.n320,
                    count_300: state.n300,
                    count_katu: state.n200,
                    count_100: state.n100,
                    count_50: state.n50,
                    count_miss: state.misses,
                };

                (GameMode::Mania, stats)
            }
        }
    }
}

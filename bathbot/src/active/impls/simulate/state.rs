use rosu_pp::{
    catch::CatchScoreState, mania::ManiaScoreState, osu::OsuScoreState, taiko::TaikoScoreState,
    Beatmap, Difficulty,
};
use rosu_v2::prelude::{GameMode, ScoreStatistics};

pub(super) enum ScoreState {
    Osu(OsuScoreState),
    Taiko(TaikoScoreState),
    Catch(CatchScoreState),
    Mania(ManiaScoreState),
}

impl ScoreState {
    pub(super) fn into_parts(
        self,
        map: Option<&Beatmap>,
    ) -> (GameMode, ScoreStatistics, ScoreStatistics) {
        match self {
            Self::Osu(state) => {
                let stats = ScoreStatistics {
                    great: state.n300,
                    ok: state.n100,
                    meh: state.n50,
                    miss: state.misses,
                    large_tick_hit: state.large_tick_hits,
                    slider_tail_hit: state.slider_end_hits,
                    ..Default::default()
                };

                let (large_tick_hit, slider_tail_hit) = map
                    .and_then(|map| {
                        Difficulty::new()
                            .calculate_for_mode::<rosu_pp::osu::Osu>(map)
                            .ok()
                    })
                    .map(|attrs| (attrs.n_large_ticks, attrs.n_sliders))
                    .unwrap_or((0, 0));

                let max_stats = ScoreStatistics {
                    great: state.n300 + state.n100 + state.n50 + state.misses,
                    large_tick_hit,
                    slider_tail_hit,
                    ..Default::default()
                };

                (GameMode::Osu, stats, max_stats)
            }
            Self::Taiko(state) => {
                let stats = ScoreStatistics {
                    great: state.n300,
                    ok: state.n100,
                    miss: state.misses,
                    ..Default::default()
                };

                let max_stats = ScoreStatistics {
                    great: state.n300 + state.n100 + state.misses,
                    ..Default::default()
                };

                (GameMode::Taiko, stats, max_stats)
            }
            Self::Catch(state) => {
                let stats = ScoreStatistics {
                    great: state.fruits,
                    good: state.tiny_droplet_misses,
                    ok: state.droplets,
                    meh: state.tiny_droplets,
                    miss: state.misses,
                    ..Default::default()
                };

                let max_stats = ScoreStatistics {
                    great: state.fruits + state.droplets + state.misses,
                    meh: state.tiny_droplets + state.tiny_droplet_misses,
                    ..Default::default()
                };

                (GameMode::Catch, stats, max_stats)
            }
            Self::Mania(state) => {
                let stats = ScoreStatistics {
                    perfect: state.n320,
                    great: state.n300,
                    good: state.n200,
                    ok: state.n100,
                    meh: state.n50,
                    miss: state.misses,
                    ..Default::default()
                };

                let max_stats = ScoreStatistics {
                    perfect: state.n320
                        + state.n300
                        + state.n200
                        + state.n100
                        + state.n50
                        + state.misses,
                    ..Default::default()
                };

                (GameMode::Mania, stats, max_stats)
            }
        }
    }
}

use rosu_pp::{
    Beatmap, Difficulty, catch::CatchHitResults, mania::ManiaHitResults, osu::OsuHitResults,
    taiko::TaikoHitResults,
};
use rosu_v2::prelude::{GameMode, ScoreStatistics};

pub(super) enum HitResults {
    Osu(OsuHitResults),
    Taiko(TaikoHitResults),
    Catch(CatchHitResults),
    Mania(ManiaHitResults),
}

impl HitResults {
    pub(super) fn into_parts(
        self,
        map: Option<&Beatmap>,
    ) -> (GameMode, ScoreStatistics, ScoreStatistics) {
        match self {
            Self::Osu(hitresults) => {
                let stats = ScoreStatistics {
                    great: hitresults.n300,
                    ok: hitresults.n100,
                    meh: hitresults.n50,
                    miss: hitresults.misses,
                    large_tick_hit: hitresults.large_tick_hits,
                    slider_tail_hit: hitresults.slider_end_hits,
                    ..Default::default()
                };

                let (large_tick_hit, slider_tail_hit) = map
                    .and_then(|map| {
                        Difficulty::new()
                            .checked_calculate_for_mode::<rosu_pp::osu::Osu>(map)
                            .ok()
                    })
                    .map(|attrs| (attrs.n_large_ticks, attrs.n_sliders))
                    .unwrap_or((0, 0));

                let max_stats = ScoreStatistics {
                    great: hitresults.n300 + hitresults.n100 + hitresults.n50 + hitresults.misses,
                    large_tick_hit,
                    slider_tail_hit,
                    ..Default::default()
                };

                (GameMode::Osu, stats, max_stats)
            }
            Self::Taiko(hitresults) => {
                let stats = ScoreStatistics {
                    great: hitresults.n300,
                    ok: hitresults.n100,
                    miss: hitresults.misses,
                    ..Default::default()
                };

                let max_stats = ScoreStatistics {
                    great: hitresults.n300 + hitresults.n100 + hitresults.misses,
                    ..Default::default()
                };

                (GameMode::Taiko, stats, max_stats)
            }
            Self::Catch(hitresults) => {
                let stats = ScoreStatistics {
                    great: hitresults.fruits,
                    good: hitresults.tiny_droplet_misses,
                    ok: hitresults.droplets,
                    meh: hitresults.tiny_droplets,
                    miss: hitresults.misses,
                    ..Default::default()
                };

                let max_stats = ScoreStatistics {
                    great: hitresults.fruits + hitresults.droplets + hitresults.misses,
                    meh: hitresults.tiny_droplets + hitresults.tiny_droplet_misses,
                    ..Default::default()
                };

                (GameMode::Catch, stats, max_stats)
            }
            Self::Mania(hitresults) => {
                let stats = ScoreStatistics {
                    perfect: hitresults.n320,
                    great: hitresults.n300,
                    good: hitresults.n200,
                    ok: hitresults.n100,
                    meh: hitresults.n50,
                    miss: hitresults.misses,
                    ..Default::default()
                };

                let max_stats = ScoreStatistics {
                    perfect: hitresults.n320
                        + hitresults.n300
                        + hitresults.n200
                        + hitresults.n100
                        + hitresults.n50
                        + hitresults.misses,
                    ..Default::default()
                };

                (GameMode::Mania, stats, max_stats)
            }
        }
    }
}

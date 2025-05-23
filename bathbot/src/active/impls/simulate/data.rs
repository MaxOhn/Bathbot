use rosu_pp::any::HitResultPriority;
use rosu_v2::{
    mods,
    prelude::{GameMod, GameMods},
};

use super::{attrs::SimulateAttributes, state::ScoreState, top_old::TopOldVersion};
use crate::{
    active::impls::SimulateMap,
    commands::osu::{TopOldCatchVersion, TopOldManiaVersion, TopOldOsuVersion, TopOldTaikoVersion},
};

pub struct SimulateData {
    pub mods: Option<GameMods>,
    pub acc: Option<f32>,
    pub n_geki: Option<u32>,
    pub n_katu: Option<u32>,
    pub n300: Option<u32>,
    pub n100: Option<u32>,
    pub n50: Option<u32>,
    pub n_miss: Option<u32>,
    pub n_slider_ends: Option<u32>,
    pub n_large_ticks: Option<u32>,
    pub combo: Option<u32>,
    pub score: Option<u32>,
    pub bpm: Option<f32>,
    pub clock_rate: Option<f64>,
    pub version: TopOldVersion,
    pub attrs: SimulateAttributes,
    pub max_combo: u32,
    pub set_on_lazer: bool,
}

impl SimulateData {
    pub(super) fn simulate(&mut self, map: &SimulateMap) -> SimulateValues {
        let mods = self
            .mods
            .as_ref()
            .map_or_else(GameMods::default, GameMods::to_owned);

        let mod_bits = mods.bits();

        if let Some(new_bpm) = self.bpm.filter(|_| self.clock_rate.is_none()) {
            let old_bpm = map.bpm();

            self.clock_rate = Some((new_bpm / old_bpm) as f64);
        }

        macro_rules! simulate {
            (
                $( $calc:ident )::+ {
                    $( $( @ $kind:tt )? $calc_fn:ident: $this_field:ident $( as $ty:ty )? ,)+
                } => {
                    mods: $mods:expr,
                    max_new: $max_new:tt,
                    $( with_diff: $with_diff:tt, )?
                    $( with_lazer: $with_lazer:tt, )?
                    $( fallible: $fallible:tt, )?
                }
            ) => {{
                let map = map.pp_map();

                if map.check_suspicion().is_ok() {
                    let mut calc = $( $calc:: )* new(map).mods($mods);
                    $( calc = simulate!(@WITH_LAZER $with_lazer calc); )?
                    simulate!(@PRIO calc $( $( $kind )? )*);

                    $(
                        if let Some(value) = self.$this_field {
                            calc = calc.$calc_fn(value $( as $ty )?);
                        }
                    )*

                    let attrs = calc.calculate();
                    $( let attrs = simulate!(@UNWRAP $fallible attrs); )?

                    let pp = attrs.pp;
                    let stars = attrs.difficulty.stars;

                    let max_new = simulate!(@MAX_NEW $max_new attrs map);

                    #[allow(unused_mut)]
                    let mut max_calc = $( $calc:: )* new(max_new).mods($mods);
                    simulate!(@PRIO max_calc $( $( $kind )? )*);

                    $( max_calc = simulate!(@WITH_DIFF $with_diff max_calc attrs); )?
                    $( max_calc = simulate!(@WITH_LAZER $with_lazer max_calc); )?

                    let attrs = max_calc.calculate();
                    $( let attrs = simulate!(@UNWRAP $fallible attrs); )?
                    let max_pp = attrs.pp;

                    (stars, pp, max_pp)
                } else {
                    (0.0, 0.0, 0.0)
                }
            }};
            ( @WITH_LAZER true $calc:ident ) => { $calc.lazer(self.set_on_lazer) };
            ( @WITH_LAZER false $calc:ident ) => { $calc };
            ( @WITH_LAZER $( $other:tt )* ) => {
                compile_error!(concat!("with_lazer must be bool; got `", $( stringify!($other) ),*, "`"))
            };
            ( @UNWRAP true $attrs:ident ) => { $attrs.unwrap() };
            ( @UNWRAP false $attrs:ident ) => { $attrs };
            ( @UNWRAP $( $other:tt )* ) => {
                compile_error!(concat!("fallible must be bool; got `", $( stringify!($other) ),*, "`"))
            };
            ( @MAX_NEW attrs $attrs:ident $map:ident ) => { $attrs };
            ( @MAX_NEW map $attrs:ident $map:ident ) => { $map };
            ( @MAX_NEW $( $other:tt )* ) => {
                compile_error!(concat!("max_new must be `attrs` or `map`; got `", $( stringify!($other) ),*, "`"))
            };
            ( @WITH_DIFF true $calc:ident $attrs:ident ) => { $calc.attributes($attrs.difficulty) };
            ( @WITH_DIFF false $calc:ident $attrs:ident ) => { $calc };
            ( @WITH_DIFF $( $other:tt )* ) => {
                compile_error!(concat!("with_diff must be bool; got `", $( stringify!($other) ),*, "`"))
            };
            ( @PRIO $calc:ident A $( $rest:tt )* ) => {
                if self.acc.is_some() {
                    $calc = $calc.hitresult_priority(HitResultPriority::Fastest);
                }
            };
            ( @PRIO $calc:ident ) => { };
            ( @PRIO $calc:ident $( $other:tt )* ) => {
                compile_error!(concat!(
                    "expected optional `@A` before rosu method name; got `", $( stringify!($other) ),* "`",
                ))
            };
        }

        let (stars, pp, max_pp) = match self.version {
            TopOldVersion::Osu(TopOldOsuVersion::May14July14) => simulate! {
                rosu_pp_older::osu_2014_may::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::July14February15) => simulate! {
                rosu_pp_older::osu_2014_july::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::February15April15) => simulate! {
                rosu_pp_older::osu_2015_february::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::April15May18) => simulate! {
                rosu_pp_older::osu_2015_april::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::May18February19) => simulate! {
                rosu_pp_older::osu_2018::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::February19January21) => simulate! {
                rosu_pp_older::osu_2019::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::January21July21) => simulate! {
                rosu_pp_older::osu_2021_january::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::July21November21) => simulate! {
                rosu_pp_older::osu_2021_july::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::November21September22) => simulate! {
                rosu_pp_older::osu_2021_november::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    accuracy: acc as f64,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::September22October24) => simulate! {
                rosu_pp_older::osu_2022::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::October24March25) => simulate! {
                rosu_pp_older::osu_2024::OsuPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    slider_end_hits: n_slider_ends,
                    small_tick_hits: n_slider_ends,
                    large_tick_hits: n_large_ticks,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                } => {
                    mods: mods.clone(),
                    max_new: map,
                    with_diff: true,
                    with_lazer: true,
                    fallible: false,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::March25Now) => simulate! {
                rosu_pp::osu::OsuPerformance {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    slider_end_hits: n_slider_ends,
                    small_tick_hits: n_slider_ends,
                    large_tick_hits: n_large_ticks,
                    clock_rate: clock_rate as f64,
                    @A accuracy: acc as f64,
                } => {
                    mods: mods.clone(),
                    max_new: attrs,
                    with_lazer: true,
                    fallible: true,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::March14September20) => simulate! {
                rosu_pp_older::taiko_ppv1::TaikoPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    misses: n_miss,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::September20September22) => simulate! {
                rosu_pp_older::taiko_2020::TaikoPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    misses: n_miss,
                    accuracy: acc as f64,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::September22October24) => simulate! {
                rosu_pp_older::taiko_2022::TaikoPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    misses: n_miss,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::March25Now) => simulate! {
                rosu_pp::taiko::TaikoPerformance {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    misses: n_miss,
                    clock_rate: clock_rate as f64,
                    @A accuracy: acc as f64,
                } => {
                    mods: mods.clone(),
                    max_new: attrs,
                    fallible: true,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::October24March25) => simulate! {
                rosu_pp_older::taiko_2024::TaikoPP {
                    combo: combo,
                    n300: n300,
                    n100: n100,
                    misses: n_miss,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                } => {
                    mods: mods.clone(),
                    max_new: map,
                    with_diff: true,
                    fallible: false,
                }
            },
            TopOldVersion::Catch(TopOldCatchVersion::March14May20) => simulate! {
                rosu_pp_older::fruits_ppv1::FruitsPP {
                    combo: combo,
                    fruits: n300,
                    droplets: n100,
                    tiny_droplets: n50,
                    misses: n_miss,
                    tiny_droplet_misses: n_katu,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Catch(TopOldCatchVersion::May20October24) => simulate! {
                rosu_pp_older::fruits_2022::FruitsPP {
                    combo: combo,
                    fruits: n300,
                    droplets: n100,
                    tiny_droplets: n50,
                    misses: n_miss,
                    tiny_droplet_misses: n_katu,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Catch(TopOldCatchVersion::October24Now) => simulate! {
                rosu_pp::catch::CatchPerformance {
                    combo: combo,
                    fruits: n300,
                    droplets: n100,
                    tiny_droplets: n50,
                    misses: n_miss,
                    tiny_droplet_misses: n_katu,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                } => {
                    mods: mods.clone(),
                    max_new: attrs,
                    fallible: true,
                }
            },
            TopOldVersion::Mania(TopOldManiaVersion::March14May18) => simulate! {
                rosu_pp_older::mania_ppv1::ManiaPP {
                    score: score,
                    accuracy: acc,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Mania(TopOldManiaVersion::May18October22) => simulate! {
                rosu_pp_older::mania_2018::ManiaPP {
                    score: score,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Mania(TopOldManiaVersion::October22October24) => simulate! {
                rosu_pp_older::mania_2022::ManiaPP {
                    n320: n_geki,
                    n200: n_katu,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                } => {
                    mods: mod_bits,
                    max_new: map,
                    with_diff: true,
                }
            },
            TopOldVersion::Mania(TopOldManiaVersion::October24Now) => simulate! {
                rosu_pp::mania::ManiaPerformance {
                    n320: n_geki,
                    n200: n_katu,
                    n300: n300,
                    n100: n100,
                    n50: n50,
                    misses: n_miss,
                    clock_rate: clock_rate as f64,
                    @A accuracy: acc as f64,
                } => {
                    mods: mods.clone(),
                    max_new: attrs,
                    with_lazer: true,
                    fallible: true,
                }
            },
        };

        let state = self.version.generate_hitresults(map.pp_map(), self);

        let combo_ratio = match state {
            Some(ScoreState::Osu(_) | ScoreState::Taiko(_) | ScoreState::Catch(_)) => {
                ComboOrRatio::Combo {
                    score: self.combo.unwrap_or(self.max_combo),
                    max: self.max_combo,
                }
            }
            Some(ScoreState::Mania(ref state))
                if matches!(
                    self.version,
                    TopOldVersion::Mania(
                        TopOldManiaVersion::October22October24 | TopOldManiaVersion::October24Now
                    )
                ) =>
            {
                match state.n300 {
                    0 => ComboOrRatio::Ratio(state.n320 as f32),
                    _ => ComboOrRatio::Ratio(state.n320 as f32 / state.n300 as f32),
                }
            }
            Some(ScoreState::Mania(_)) | None => ComboOrRatio::Neither,
        };

        let clock_rate = self
            .clock_rate
            .filter(|_| {
                matches!(
                    self.version,
                    TopOldVersion::Osu(
                        TopOldOsuVersion::September22October24
                            | TopOldOsuVersion::October24March25
                            | TopOldOsuVersion::March25Now
                    ) | TopOldVersion::Taiko(
                        TopOldTaikoVersion::September22October24
                            | TopOldTaikoVersion::October24March25
                            | TopOldTaikoVersion::March25Now
                    ) | TopOldVersion::Catch(
                        TopOldCatchVersion::May20October24 | TopOldCatchVersion::October24Now
                    ) | TopOldVersion::Mania(
                        TopOldManiaVersion::October22October24 | TopOldManiaVersion::October24Now
                    )
                )
            })
            .or_else(|| {
                self.mods.as_ref().and_then(|mods| {
                    mods.contains_any(mods!(DT HT))
                        .then(|| mods.clock_rate().unwrap_or(1.0))
                })
            });

        let score_state = match state {
            Some(state @ (ScoreState::Osu(_) | ScoreState::Taiko(_) | ScoreState::Catch(_))) => {
                StateOrScore::State(state)
            }
            Some(state @ ScoreState::Mania(_))
                if matches!(
                    self.version,
                    TopOldVersion::Mania(
                        TopOldManiaVersion::October22October24 | TopOldManiaVersion::October24Now
                    )
                ) =>
            {
                StateOrScore::State(state)
            }
            Some(ScoreState::Mania(_)) => match self.score {
                Some(score) => StateOrScore::Score(score),
                None => {
                    let mult = self.mods.as_ref().map(score_multiplier).unwrap_or(1.0);

                    StateOrScore::Score((1_000_000.0 * mult) as u32)
                }
            },
            None => StateOrScore::Neither,
        };

        SimulateValues {
            stars: stars as f32,
            pp: pp as f32,
            max_pp: max_pp as f32,
            clock_rate,
            combo_ratio,
            score_state,
        }
    }
}

fn score_multiplier(mods: &GameMods) -> f32 {
    mods.iter()
        .map(|gamemod| match gamemod {
            GameMod::HalfTimeOsu(_) | GameMod::HalfTimeTaiko(_) | GameMod::HalfTimeCatch(_) => 0.3,
            GameMod::EasyOsu(_)
            | GameMod::NoFailOsu(_)
            | GameMod::EasyTaiko(_)
            | GameMod::NoFailTaiko(_)
            | GameMod::EasyCatch(_)
            | GameMod::NoFailCatch(_)
            | GameMod::EasyMania(_)
            | GameMod::NoFailMania(_)
            | GameMod::HalfTimeMania(_) => 0.5,
            GameMod::SpunOutOsu(_) => 0.9,
            GameMod::HardRockOsu(_)
            | GameMod::HiddenOsu(_)
            | GameMod::HardRockTaiko(_)
            | GameMod::HiddenTaiko(_)
            | GameMod::DoubleTimeCatch(_)
            | GameMod::NightcoreCatch(_)
            | GameMod::HiddenCatch(_) => 1.06,
            GameMod::DoubleTimeOsu(_)
            | GameMod::NightcoreOsu(_)
            | GameMod::FlashlightOsu(_)
            | GameMod::DoubleTimeTaiko(_)
            | GameMod::NightcoreTaiko(_)
            | GameMod::FlashlightTaiko(_)
            | GameMod::HardRockCatch(_)
            | GameMod::FlashlightCatch(_) => 1.12,
            _ => 1.0,
        })
        .product()
}

pub(super) struct SimulateValues {
    pub stars: f32,
    pub pp: f32,
    pub max_pp: f32,
    pub clock_rate: Option<f64>,
    pub combo_ratio: ComboOrRatio,
    pub score_state: StateOrScore,
}

pub(super) enum StateOrScore {
    Score(u32),
    State(ScoreState),
    /// The map was too suspicious
    Neither,
}

pub(super) enum ComboOrRatio {
    Combo { score: u32, max: u32 },
    Ratio(f32),
    Neither,
}

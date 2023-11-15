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
    pub combo: Option<u32>,
    pub score: Option<u32>,
    pub bpm: Option<f32>,
    pub clock_rate: Option<f32>,
    pub version: TopOldVersion,
    pub attrs: SimulateAttributes,
    pub original_attrs: SimulateAttributes,
    pub is_convert: Option<bool>,
    pub max_combo: u32,
}

impl SimulateData {
    pub fn set_acc(&mut self, acc: Option<f32>) {
        self.acc = acc.map(|acc| acc.clamp(0.0, 100.0));
    }

    pub(super) fn simulate(&mut self, map: &SimulateMap) -> SimulateValues {
        let mods = self.mods.as_ref().map_or(0, GameMods::bits);

        if let Some(new_bpm) = self.bpm.filter(|_| self.clock_rate.is_none()) {
            let old_bpm = map.bpm();

            self.clock_rate = Some(new_bpm / old_bpm);
        }

        macro_rules! simulate {
            (
                $( $calc:ident )::+ {
                    $( $calc_method:ident: $this_field:ident $( as $ty:ty )? ,)+
                }
            ) => {{
                let mut calc = $( $calc:: )* new(map.pp_map()).mods(mods);

                $(
                    if let Some(value) = self.$this_field {
                        calc = calc.$calc_method(value $( as $ty )?);
                    }
                )*

                let attrs = calc.calculate();

                let pp = attrs.pp;
                let stars = attrs.difficulty.stars;

                let max_pp = $( $calc:: )* new(map.pp_map())
                    .attributes(attrs)
                    .mods(mods)
                    .calculate()
                    .pp;

                (stars, pp, max_pp)
            }}
        }

        let (stars, pp, max_pp) = match self.version {
            TopOldVersion::Osu(TopOldOsuVersion::May14July14) => simulate! {
                rosu_pp_older::osu_2014_may::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::July14February15) => simulate! {
                rosu_pp_older::osu_2014_july::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::February15April15) => simulate! {
                rosu_pp_older::osu_2015_february::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::April15May18) => simulate! {
                rosu_pp_older::osu_2015_april::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::May18February19) => simulate! {
                rosu_pp_older::osu_2018::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::February19January21) => simulate! {
                rosu_pp_older::osu_2019::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::January21July21) => simulate! {
                rosu_pp_older::osu_2021_january::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::July21November21) => simulate! {
                rosu_pp_older::osu_2021_july::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::November21September22) => simulate! {
                rosu_pp_older::osu_2021_november::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    misses: n_miss as usize,
                    accuracy: acc as f64,
                }
            },
            TopOldVersion::Osu(TopOldOsuVersion::September22Now) => simulate! {
                rosu_pp::OsuPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    n_misses: n_miss as usize,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::March14September20) => simulate! {
                rosu_pp_older::taiko_ppv1::TaikoPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    misses: n_miss as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::September20September22) => simulate! {
                rosu_pp_older::taiko_2020::TaikoPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    misses: n_miss as usize,
                    accuracy: acc as f64,
                }
            },
            TopOldVersion::Taiko(TopOldTaikoVersion::September22Now) => simulate! {
                rosu_pp::TaikoPP {
                    combo: combo as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n_misses: n_miss as usize,
                    is_convert: is_convert,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                }
            },
            TopOldVersion::Catch(TopOldCatchVersion::March14May20) => simulate! {
                rosu_pp_older::fruits_ppv1::FruitsPP {
                    combo: combo as usize,
                    fruits: n300 as usize,
                    droplets: n100 as usize,
                    tiny_droplets: n50 as usize,
                    misses: n_miss as usize,
                    tiny_droplet_misses: n_katu as usize,
                    accuracy: acc,
                }
            },
            TopOldVersion::Catch(TopOldCatchVersion::May20Now) => simulate! {
                rosu_pp::CatchPP {
                    combo: combo as usize,
                    fruits: n300 as usize,
                    droplets: n100 as usize,
                    tiny_droplets: n50 as usize,
                    misses: n_miss as usize,
                    tiny_droplet_misses: n_katu as usize,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                }
            },
            TopOldVersion::Mania(TopOldManiaVersion::March14May18) => simulate! {
                rosu_pp_older::mania_ppv1::ManiaPP {
                    score: score,
                    accuracy: acc,
                }
            },
            TopOldVersion::Mania(TopOldManiaVersion::May18October22) => simulate! {
                rosu_pp_older::mania_2018::ManiaPP {
                    score: score,
                }
            },
            TopOldVersion::Mania(TopOldManiaVersion::October22Now) => simulate! {
                rosu_pp::ManiaPP {
                    n320: n_geki as usize,
                    n200: n_katu as usize,
                    n300: n300 as usize,
                    n100: n100 as usize,
                    n50: n50 as usize,
                    n_misses: n_miss as usize,
                    is_convert: is_convert,
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                }
            },
        };

        let state = self.version.generate_hitresults(map.pp_map(), self);

        let combo_ratio = match state {
            ScoreState::Osu(_) | ScoreState::Taiko(_) | ScoreState::Catch(_) => {
                ComboOrRatio::Combo {
                    score: self.combo.unwrap_or(self.max_combo),
                    max: self.max_combo,
                }
            }
            ScoreState::Mania(ref state)
                if matches!(
                    self.version,
                    TopOldVersion::Mania(TopOldManiaVersion::October22Now)
                ) =>
            {
                match state.n300 {
                    0 => ComboOrRatio::Ratio(state.n320 as f32),
                    _ => ComboOrRatio::Ratio(state.n320 as f32 / state.n300 as f32),
                }
            }
            ScoreState::Mania(_) => ComboOrRatio::Neither,
        };

        let clock_rate = self
            .clock_rate
            .filter(|_| {
                matches!(
                    self.version,
                    TopOldVersion::Osu(TopOldOsuVersion::September22Now)
                        | TopOldVersion::Taiko(TopOldTaikoVersion::September22Now)
                        | TopOldVersion::Catch(TopOldCatchVersion::May20Now)
                        | TopOldVersion::Mania(TopOldManiaVersion::October22Now)
                )
            })
            .or_else(|| {
                self.mods.as_ref().and_then(|mods| {
                    mods.contains_any(mods!(DT HT))
                        .then(|| mods.clock_rate().unwrap_or(1.0))
                })
            });

        let score_state = match state {
            state @ (ScoreState::Osu(_) | ScoreState::Taiko(_) | ScoreState::Catch(_)) => {
                StateOrScore::State(state)
            }
            state @ ScoreState::Mania(_)
                if self.version == TopOldVersion::Mania(TopOldManiaVersion::October22Now) =>
            {
                StateOrScore::State(state)
            }
            ScoreState::Mania(_) => match self.score {
                Some(score) => StateOrScore::Score(score),
                None => {
                    let mult = self.mods.as_ref().map(score_multiplier).unwrap_or(1.0);

                    StateOrScore::Score((1_000_000.0 * mult) as u32)
                }
            },
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
    pub clock_rate: Option<f32>,
    pub combo_ratio: ComboOrRatio,
    pub score_state: StateOrScore,
}

pub(super) enum StateOrScore {
    Score(u32),
    State(ScoreState),
}

pub(super) enum ComboOrRatio {
    Combo { score: u32, max: u32 },
    Ratio(f32),
    Neither,
}

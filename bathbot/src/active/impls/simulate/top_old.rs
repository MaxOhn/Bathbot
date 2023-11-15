use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_pp::{
    catch::CatchScoreState, mania::ManiaScoreState, osu::OsuScoreState, taiko::TaikoScoreState,
    AnyPP, Beatmap,
};
use twilight_model::channel::message::{
    component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption},
    Component,
};

use super::{data::SimulateData, state::ScoreState};
use crate::commands::osu::{
    TopOldCatchVersion, TopOldManiaVersion, TopOldOsuVersion, TopOldTaikoVersion,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TopOldVersion {
    Osu(TopOldOsuVersion),
    Taiko(TopOldTaikoVersion),
    Catch(TopOldCatchVersion),
    Mania(TopOldManiaVersion),
}

impl TopOldVersion {
    pub fn from_menu_str(s: &str) -> Option<Self> {
        let version = match s {
            "sim_osu_september22_now" => Self::Osu(TopOldOsuVersion::September22Now),
            "sim_osu_november21_september22" => Self::Osu(TopOldOsuVersion::November21September22),
            "sim_osu_july21_november21" => Self::Osu(TopOldOsuVersion::July21November21),
            "sim_osu_january21_july21" => Self::Osu(TopOldOsuVersion::January21July21),
            "sim_osu_february19_january21" => Self::Osu(TopOldOsuVersion::February19January21),
            "sim_osu_may18_february19" => Self::Osu(TopOldOsuVersion::May18February19),
            "sim_osu_april15_may18" => Self::Osu(TopOldOsuVersion::April15May18),
            "sim_osu_february15_april15" => Self::Osu(TopOldOsuVersion::February15April15),
            "sim_osu_july14_february15" => Self::Osu(TopOldOsuVersion::July14February15),
            "sim_osu_may14_july14" => Self::Osu(TopOldOsuVersion::May14July14),
            "sim_taiko_september22_now" => Self::Taiko(TopOldTaikoVersion::September22Now),
            "sim_taiko_september20_september22" => {
                Self::Taiko(TopOldTaikoVersion::September20September22)
            }
            "sim_taiko_march14_september20" => Self::Taiko(TopOldTaikoVersion::March14September20),
            "sim_catch_may20_now" => Self::Catch(TopOldCatchVersion::May20Now),
            "sim_catch_march14_may20" => Self::Catch(TopOldCatchVersion::March14May20),
            "sim_mania_october22_now" => Self::Mania(TopOldManiaVersion::October22Now),
            "sim_mania_may18_october22" => Self::Mania(TopOldManiaVersion::May18October22),
            "sim_mania_march14_may18" => Self::Mania(TopOldManiaVersion::March14May18),
            _ => return None,
        };

        Some(version)
    }

    pub fn components(self) -> Vec<Component> {
        macro_rules! versions {
                ( $( $label:literal, $value:literal, $version:ident = $ty:ident :: $variant:ident ;)* ) => {
                    vec![
                        $(
                            SelectMenuOption {
                                default: $version == $ty::$variant,
                                description: None,
                                emoji: None,
                                label: $label.to_owned(),
                                value: $value.to_owned(),
                            },
                        )*
                    ]
                }
            }

        macro_rules! button {
            ($custom_id:literal, $label:literal, $style:ident) => {
                Button {
                    custom_id: Some($custom_id.to_owned()),
                    disabled: false,
                    emoji: None,
                    label: Some($label.to_owned()),
                    style: ButtonStyle::$style,
                    url: None,
                }
            };
        }

        let (upper, bottom, version) = match self {
            Self::Osu(version) => {
                let mods = button!("sim_mods", "Mods", Primary);
                let combo = button!("sim_combo", "Combo", Primary);
                let acc = button!("sim_acc", "Accuracy", Primary);

                let mut upper = vec![
                    Component::Button(mods),
                    Component::Button(combo),
                    Component::Button(acc),
                ];

                if let TopOldOsuVersion::September22Now = version {
                    let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                    upper.push(Component::Button(clock_rate));
                }

                let attrs = button!("sim_attrs", "Attributes", Primary);
                upper.push(Component::Button(attrs));

                let n300 = button!("sim_n300", "n300", Secondary);
                let n100 = button!("sim_n100", "n100", Secondary);
                let n50 = button!("sim_n50", "n50", Secondary);
                let n_miss = button!("sim_miss", "Misses", Danger);

                let bottom = vec![
                    Component::Button(n300),
                    Component::Button(n100),
                    Component::Button(n50),
                    Component::Button(n_miss),
                ];

                let options = versions![
                    "September 2022 - Now", "sim_osu_september22_now", version = TopOldOsuVersion::September22Now;
                    "November 2021 - September 2022", "sim_osu_november21_september22", version = TopOldOsuVersion::November21September22;
                    "July 2021 - November 2021", "sim_osu_july21_november21", version = TopOldOsuVersion::July21November21;
                    "January 2021 - July 2021", "sim_osu_january21_july21", version = TopOldOsuVersion::January21July21;
                    "February 2019 - January 2021", "sim_osu_february19_january21", version = TopOldOsuVersion::February19January21;
                    "May 2018 - February 2019", "sim_osu_may18_february19", version = TopOldOsuVersion::May18February19;
                    "April 2015 - May 2018", "sim_osu_april15_may18", version = TopOldOsuVersion::April15May18;
                    "February 2015 - April 2015", "sim_osu_february15_april15", version = TopOldOsuVersion::February15April15;
                    "July 2014 - February 2015", "sim_osu_july14_february15", version = TopOldOsuVersion::July14February15;
                    "May 2014 - July 2014", "sim_osu_may14_july14", version = TopOldOsuVersion::May14July14;
                ];

                let version = SelectMenu {
                    custom_id: "sim_osu_version".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options,
                    placeholder: None,
                };

                (upper, Some(bottom), Component::SelectMenu(version))
            }
            Self::Taiko(version) => {
                let mods = button!("sim_mods", "Mods", Primary);
                let combo = button!("sim_combo", "Combo", Primary);
                let acc = button!("sim_acc", "Accuracy", Primary);

                let mut upper = vec![
                    Component::Button(mods),
                    Component::Button(combo),
                    Component::Button(acc),
                ];

                if let TopOldTaikoVersion::September22Now = version {
                    let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                    upper.push(Component::Button(clock_rate));
                }

                let attrs = button!("sim_attrs", "Attributes", Primary);
                upper.push(Component::Button(attrs));

                let n300 = button!("sim_n300", "n300", Secondary);
                let n100 = button!("sim_n100", "n100", Secondary);
                let n_miss = button!("sim_miss", "Misses", Danger);

                let bottom = vec![
                    Component::Button(n300),
                    Component::Button(n100),
                    Component::Button(n_miss),
                ];

                let options = versions![
                    "September 2022 - Now", "sim_taiko_september22_now", version = TopOldTaikoVersion::September22Now;
                    "September 2020 - September 2022","sim_taiko_september20_september22", version = TopOldTaikoVersion::September20September22;
                    "March 2014 - September 2020", "sim_taiko_march14_september20", version = TopOldTaikoVersion::March14September20;
                ];

                let version = SelectMenu {
                    custom_id: "sim_taiko_version".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options,
                    placeholder: None,
                };

                (upper, Some(bottom), Component::SelectMenu(version))
            }
            Self::Catch(version) => {
                let mods = button!("sim_mods", "Mods", Primary);
                let combo = button!("sim_combo", "Combo", Primary);
                let acc = button!("sim_acc", "Accuracy", Primary);

                let mut upper = vec![
                    Component::Button(mods),
                    Component::Button(combo),
                    Component::Button(acc),
                ];

                if let TopOldCatchVersion::May20Now = version {
                    let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                    upper.push(Component::Button(clock_rate));
                }

                let attrs = button!("sim_attrs", "Attributes", Primary);
                upper.push(Component::Button(attrs));

                let n_fruits = button!("sim_n300", "Fruits", Secondary);
                let n_droplets = button!("sim_n100", "Droplets", Secondary);
                let n_tiny_droplets = button!("sim_n50", "Tiny droplets", Secondary);
                let n_tiny_droplet_misses = button!("sim_katu", "Tiny droplet misses", Secondary);
                let n_misses = button!("sim_miss", "Misses", Danger);

                let bottom = vec![
                    Component::Button(n_fruits),
                    Component::Button(n_droplets),
                    Component::Button(n_tiny_droplets),
                    Component::Button(n_misses),
                    Component::Button(n_tiny_droplet_misses),
                ];

                let options = versions![
                    "May 2020 - Now", "sim_catch_may20_now", version = TopOldCatchVersion::May20Now;
                    "March 2014 - May 2020", "sim_catch_march14_may20", version = TopOldCatchVersion::March14May20;
                ];

                let version = SelectMenu {
                    custom_id: "sim_catch_version".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options,
                    placeholder: None,
                };

                (upper, Some(bottom), Component::SelectMenu(version))
            }
            Self::Mania(version) => {
                let (upper, bottom) = match version {
                    TopOldManiaVersion::March14May18 | TopOldManiaVersion::May18October22 => {
                        let mods = button!("sim_mods", "Mods", Primary);
                        let score = button!("sim_score", "Score", Primary);
                        let attrs = button!("sim_attrs", "Attributes", Primary);

                        let upper = vec![
                            Component::Button(mods),
                            Component::Button(score),
                            Component::Button(attrs),
                        ];

                        (upper, None)
                    }
                    TopOldManiaVersion::October22Now => {
                        let mods = button!("sim_mods", "Mods", Primary);
                        let acc = button!("sim_acc", "Accuracy", Primary);
                        let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                        let attrs = button!("sim_attrs", "Attributes", Primary);
                        let n_miss = button!("sim_miss", "Misses", Danger);

                        let upper = vec![
                            Component::Button(mods),
                            Component::Button(acc),
                            Component::Button(clock_rate),
                            Component::Button(attrs),
                            Component::Button(n_miss),
                        ];

                        let n320 = button!("sim_geki", "n320", Secondary);
                        let n300 = button!("sim_n300", "n300", Secondary);
                        let n200 = button!("sim_katu", "n200", Secondary);
                        let n100 = button!("sim_n100", "n100", Secondary);
                        let n50 = button!("sim_n50", "n50", Secondary);

                        let bottom = vec![
                            Component::Button(n320),
                            Component::Button(n300),
                            Component::Button(n200),
                            Component::Button(n100),
                            Component::Button(n50),
                        ];

                        (upper, Some(bottom))
                    }
                };

                let options = versions![
                    "October 2022 - Now", "sim_mania_october22_now", version = TopOldManiaVersion::October22Now;
                    "May 2018 - October 2022", "sim_mania_may18_october22", version = TopOldManiaVersion::May18October22;
                    "March 2014 - May 2018", "sim_mania_march14_may18", version = TopOldManiaVersion::March14May18;
                ];

                let version = SelectMenu {
                    custom_id: "sim_mania_version".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options,
                    placeholder: None,
                };

                (upper, bottom, Component::SelectMenu(version))
            }
        };

        let upper = Component::ActionRow(ActionRow { components: upper });
        let version = Component::ActionRow(ActionRow {
            components: vec![version],
        });

        match bottom.map(|components| ActionRow { components }) {
            Some(bottom) => vec![upper, Component::ActionRow(bottom), version],
            None => vec![upper, version],
        }
    }

    pub(super) fn generate_hitresults(self, map: &Beatmap, data: &SimulateData) -> ScoreState {
        let mut calc = AnyPP::new(map);

        if let Some(acc) = data.acc {
            calc = calc.accuracy(acc as f64);
        }

        if let Some(n_geki) = data.n_geki {
            calc = calc.n_geki(n_geki as usize);
        }

        if let Some(n_katu) = data.n_katu {
            calc = calc.n_katu(n_katu as usize);
        }

        if let Some(n300) = data.n300 {
            calc = calc.n300(n300 as usize);
        }

        if let Some(n100) = data.n100 {
            calc = calc.n100(n100 as usize);
        }

        if let Some(n50) = data.n50 {
            calc = calc.n50(n50 as usize);
        }

        if let Some(n_miss) = data.n_miss {
            calc = calc.n_misses(n_miss as usize);
        }

        let state = calc.generate_state();

        match self {
            Self::Osu(_) => ScoreState::Osu(OsuScoreState {
                max_combo: state.max_combo,
                n300: state.n300,
                n100: state.n100,
                n50: state.n50,
                n_misses: state.n_misses,
            }),
            Self::Taiko(_) => ScoreState::Taiko(TaikoScoreState {
                max_combo: state.max_combo,
                n300: state.n300,
                n100: state.n100,
                n_misses: state.n_misses,
            }),
            Self::Catch(_) => ScoreState::Catch(CatchScoreState {
                max_combo: state.max_combo,
                n_fruits: state.n300,
                n_droplets: state.n100,
                n_tiny_droplets: state.n50,
                n_tiny_droplet_misses: state.n_katu,
                n_misses: state.n_misses,
            }),
            Self::Mania(_) => ScoreState::Mania(ManiaScoreState {
                n320: state.n_geki,
                n300: state.n300,
                n200: state.n_katu,
                n100: state.n100,
                n50: state.n50,
                n_misses: state.n_misses,
            }),
        }
    }
}

impl Display for TopOldVersion {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Osu(version) => {
                f.write_str("osu! version ")?;

                match version {
                    TopOldOsuVersion::May14July14 => f.write_str("may 2014 - july 2014"),
                    TopOldOsuVersion::July14February15 => f.write_str("july 2014 - february 2015"),
                    TopOldOsuVersion::February15April15 => {
                        f.write_str("february 2015 - april 2015")
                    }
                    TopOldOsuVersion::April15May18 => f.write_str("april 2015 - may 2018"),
                    TopOldOsuVersion::May18February19 => f.write_str("may 2018 - february 2019"),
                    TopOldOsuVersion::February19January21 => {
                        f.write_str("february 2019 - january 2021")
                    }
                    TopOldOsuVersion::January21July21 => f.write_str("january 2021 - july 2021"),
                    TopOldOsuVersion::July21November21 => f.write_str("july 2021 - november 2021"),
                    TopOldOsuVersion::November21September22 => {
                        f.write_str("november 2021 - september 2022")
                    }
                    TopOldOsuVersion::September22Now => f.write_str("september 2022 - now"),
                }
            }
            Self::Taiko(version) => {
                f.write_str("Taiko version ")?;

                match version {
                    TopOldTaikoVersion::March14September20 => {
                        f.write_str("march 2014 - september 2020")
                    }
                    TopOldTaikoVersion::September20September22 => {
                        f.write_str("september 2020 - september 2022")
                    }
                    TopOldTaikoVersion::September22Now => f.write_str("september 2022 - now"),
                }
            }
            Self::Catch(version) => {
                f.write_str("Catch version ")?;

                match version {
                    TopOldCatchVersion::March14May20 => f.write_str("march 2014 - may 2020"),
                    TopOldCatchVersion::May20Now => f.write_str("may 2020 - now"),
                }
            }
            Self::Mania(version) => {
                f.write_str("Mania version ")?;

                match version {
                    TopOldManiaVersion::March14May18 => f.write_str("march 2014 - may 2018"),
                    TopOldManiaVersion::May18October22 => f.write_str("may 2018 - october 2022"),
                    TopOldManiaVersion::October22Now => f.write_str("october 2022 - now"),
                }
            }
        }
    }
}

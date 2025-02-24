use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_pp::{
    Beatmap, Performance, catch::CatchScoreState, mania::ManiaScoreState, osu::OsuScoreState,
    taiko::TaikoScoreState,
};
use twilight_model::channel::message::{
    Component,
    component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption, SelectMenuType},
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
            "sim_osu_october24_now" => Self::Osu(TopOldOsuVersion::October24Now),
            "sim_osu_september22_october24" => Self::Osu(TopOldOsuVersion::September22October24),
            "sim_osu_november21_september22" => Self::Osu(TopOldOsuVersion::November21September22),
            "sim_osu_july21_november21" => Self::Osu(TopOldOsuVersion::July21November21),
            "sim_osu_january21_july21" => Self::Osu(TopOldOsuVersion::January21July21),
            "sim_osu_february19_january21" => Self::Osu(TopOldOsuVersion::February19January21),
            "sim_osu_may18_february19" => Self::Osu(TopOldOsuVersion::May18February19),
            "sim_osu_april15_may18" => Self::Osu(TopOldOsuVersion::April15May18),
            "sim_osu_february15_april15" => Self::Osu(TopOldOsuVersion::February15April15),
            "sim_osu_july14_february15" => Self::Osu(TopOldOsuVersion::July14February15),
            "sim_osu_may14_july14" => Self::Osu(TopOldOsuVersion::May14July14),
            "sim_taiko_october24_now" => Self::Taiko(TopOldTaikoVersion::October24Now),
            "sim_taiko_september22_october24" => {
                Self::Taiko(TopOldTaikoVersion::September22October24)
            }
            "sim_taiko_september20_september22" => {
                Self::Taiko(TopOldTaikoVersion::September20September22)
            }
            "sim_taiko_march14_september20" => Self::Taiko(TopOldTaikoVersion::March14September20),
            "sim_catch_october24_now" => Self::Catch(TopOldCatchVersion::October24Now),
            "sim_catch_may20_october24" => Self::Catch(TopOldCatchVersion::May20October24),
            "sim_catch_march14_may20" => Self::Catch(TopOldCatchVersion::March14May20),
            "sim_mania_october24_now" => Self::Mania(TopOldManiaVersion::October24Now),
            "sim_mania_october22_october24" => Self::Mania(TopOldManiaVersion::October22October24),
            "sim_mania_may18_october22" => Self::Mania(TopOldManiaVersion::May18October22),
            "sim_mania_march14_may18" => Self::Mania(TopOldManiaVersion::March14May18),
            _ => return None,
        };

        Some(version)
    }

    pub fn components(self, set_on_lazer: bool) -> Vec<Component> {
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
                    sku_id: None,
                }
            };
        }

        let (upper, middle, bottom, version) = match self {
            Self::Osu(version) => {
                let mods = button!("sim_mods", "Mods", Primary);
                let combo = button!("sim_combo", "Combo", Primary);
                let acc = button!("sim_acc", "Accuracy", Primary);

                let mut upper = vec![
                    Component::Button(mods),
                    Component::Button(combo),
                    Component::Button(acc),
                ];

                match version {
                    TopOldOsuVersion::September22October24 | TopOldOsuVersion::October24Now => {
                        let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                        upper.push(Component::Button(clock_rate));
                    }
                    _ => {}
                }

                let attrs = button!("sim_attrs", "Attributes", Primary);
                upper.push(Component::Button(attrs));

                let n300 = button!("sim_n300", "n300", Secondary);
                let n100 = button!("sim_n100", "n100", Secondary);
                let n50 = button!("sim_n50", "n50", Secondary);
                let n_miss = button!("sim_miss", "Misses", Danger);

                let lazer = Button {
                    disabled: set_on_lazer,
                    ..button!("sim_lazer", "Lazer", Primary)
                };

                let stable = Button {
                    disabled: !set_on_lazer,
                    ..button!("sim_stable", "Stable", Primary)
                };

                let n_slider_ends = Button {
                    disabled: !set_on_lazer,
                    ..button!("sim_slider_ends", "Slider ends", Secondary)
                };

                let n_large_ticks = Button {
                    disabled: !set_on_lazer,
                    ..button!("sim_large_ticks", "Large ticks", Secondary)
                };

                let middle = vec![
                    Component::Button(n300),
                    Component::Button(n100),
                    Component::Button(n50),
                    Component::Button(n_miss),
                ];

                let bottom = vec![
                    Component::Button(n_slider_ends),
                    Component::Button(n_large_ticks),
                    Component::Button(lazer),
                    Component::Button(stable),
                ];

                let options = versions![
                    "October 2024 - Now", "sim_osu_october24_now", version = TopOldOsuVersion::October24Now;
                    "September 2022 - October 2024", "sim_osu_september22_october24", version = TopOldOsuVersion::September22October24;
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
                    options: Some(options),
                    placeholder: None,
                    channel_types: None,
                    default_values: None,
                    kind: SelectMenuType::Text,
                };

                (
                    upper,
                    Some(middle),
                    Some(bottom),
                    Component::SelectMenu(version),
                )
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

                match version {
                    TopOldTaikoVersion::September22October24 | TopOldTaikoVersion::October24Now => {
                        let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                        upper.push(Component::Button(clock_rate));
                    }
                    _ => {}
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
                    "October 2024 - Now", "sim_taiko_october24_now", version = TopOldTaikoVersion::October24Now;
                    "September 2022 - October 2024", "sim_taiko_september22_october24", version = TopOldTaikoVersion::September22October24;
                    "September 2020 - September 2022","sim_taiko_september20_september22", version = TopOldTaikoVersion::September20September22;
                    "March 2014 - September 2020", "sim_taiko_march14_september20", version = TopOldTaikoVersion::March14September20;
                ];

                let version = SelectMenu {
                    custom_id: "sim_taiko_version".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options: Some(options),
                    placeholder: None,
                    channel_types: None,
                    default_values: None,
                    kind: SelectMenuType::Text,
                };

                (upper, Some(bottom), None, Component::SelectMenu(version))
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

                match version {
                    TopOldCatchVersion::May20October24 | TopOldCatchVersion::October24Now => {
                        let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                        upper.push(Component::Button(clock_rate));
                    }
                    _ => {}
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
                    "October 2024 - Now", "sim_catch_october24_now", version = TopOldCatchVersion::October24Now;
                    "May 2020 - October 2024", "sim_catch_may20_october24", version = TopOldCatchVersion::May20October24;
                    "March 2014 - May 2020", "sim_catch_march14_may20", version = TopOldCatchVersion::March14May20;
                ];

                let version = SelectMenu {
                    custom_id: "sim_catch_version".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options: Some(options),
                    placeholder: None,
                    channel_types: None,
                    default_values: None,
                    kind: SelectMenuType::Text,
                };

                (upper, Some(bottom), None, Component::SelectMenu(version))
            }
            Self::Mania(version) => {
                let (upper, middle, bottom) = match version {
                    TopOldManiaVersion::March14May18 | TopOldManiaVersion::May18October22 => {
                        let mods = button!("sim_mods", "Mods", Primary);
                        let score = button!("sim_score", "Score", Primary);
                        let attrs = button!("sim_attrs", "Attributes", Primary);

                        let upper = vec![
                            Component::Button(mods),
                            Component::Button(score),
                            Component::Button(attrs),
                        ];

                        (upper, None, None)
                    }
                    TopOldManiaVersion::October22October24 | TopOldManiaVersion::October24Now => {
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

                        let middle = vec![
                            Component::Button(n320),
                            Component::Button(n300),
                            Component::Button(n200),
                            Component::Button(n100),
                            Component::Button(n50),
                        ];

                        let bottom = if let TopOldManiaVersion::October24Now = version {
                            Some(vec![
                                Component::Button(Button {
                                    disabled: set_on_lazer,
                                    ..button!("sim_lazer", "Lazer", Primary)
                                }),
                                Component::Button(Button {
                                    disabled: !set_on_lazer,
                                    ..button!("sim_stable", "Stable", Primary)
                                }),
                            ])
                        } else {
                            None
                        };

                        (upper, Some(middle), bottom)
                    }
                };

                let options = versions![
                    "October 2024 - Now", "sim_mania_october24_now", version = TopOldManiaVersion::October24Now;
                    "October 2022 - October 2024", "sim_mania_october22_october24", version = TopOldManiaVersion::October22October24;
                    "May 2018 - October 2022", "sim_mania_may18_october22", version = TopOldManiaVersion::May18October22;
                    "March 2014 - May 2018", "sim_mania_march14_may18", version = TopOldManiaVersion::March14May18;
                ];

                let version = SelectMenu {
                    custom_id: "sim_mania_version".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options: Some(options),
                    placeholder: None,
                    channel_types: None,
                    default_values: None,
                    kind: SelectMenuType::Text,
                };

                (upper, middle, bottom, Component::SelectMenu(version))
            }
        };

        let upper = Component::ActionRow(ActionRow { components: upper });
        let version = Component::ActionRow(ActionRow {
            components: vec![version],
        });

        let mut components = Vec::new();
        components.push(upper);

        if let Some(middle) = middle {
            components.push(Component::ActionRow(ActionRow { components: middle }));
        }

        if let Some(bottom) = bottom {
            components.push(Component::ActionRow(ActionRow { components: bottom }));
        }

        components.push(version);

        components
    }

    pub(super) fn generate_hitresults(self, map: &Beatmap, data: &SimulateData) -> ScoreState {
        let mut calc = Performance::new(map).lazer(data.set_on_lazer);

        if let Some(acc) = data.acc {
            calc = calc.accuracy(acc as f64);
        }

        if let Some(n_geki) = data.n_geki {
            calc = calc.n_geki(n_geki);
        }

        if let Some(n_katu) = data.n_katu {
            calc = calc.n_katu(n_katu);
        }

        if let Some(n300) = data.n300 {
            calc = calc.n300(n300);
        }

        if let Some(n100) = data.n100 {
            calc = calc.n100(n100);
        }

        if let Some(n50) = data.n50 {
            calc = calc.n50(n50);
        }

        if let Some(n_miss) = data.n_miss {
            calc = calc.misses(n_miss);
        }

        if let Some(n_slider_ends) = data.n_slider_ends {
            calc = calc
                .slider_end_hits(n_slider_ends)
                .small_tick_hits(n_slider_ends);
        }

        if let Some(n_large_ticks) = data.n_large_ticks {
            calc = calc.large_tick_hits(n_large_ticks);
        }

        let state = calc.generate_state();

        match self {
            Self::Osu(_) => ScoreState::Osu(OsuScoreState {
                max_combo: state.max_combo,
                n300: state.n300,
                n100: state.n100,
                n50: state.n50,
                misses: state.misses,
                large_tick_hits: state.osu_large_tick_hits,
                small_tick_hits: state.osu_small_tick_hits,
                slider_end_hits: state.slider_end_hits,
            }),
            Self::Taiko(_) => ScoreState::Taiko(TaikoScoreState {
                max_combo: state.max_combo,
                n300: state.n300,
                n100: state.n100,
                misses: state.misses,
            }),
            Self::Catch(_) => ScoreState::Catch(CatchScoreState {
                max_combo: state.max_combo,
                fruits: state.n300,
                droplets: state.n100,
                tiny_droplets: state.n50,
                tiny_droplet_misses: state.n_katu,
                misses: state.misses,
            }),
            Self::Mania(_) => ScoreState::Mania(ManiaScoreState {
                n320: state.n_geki,
                n300: state.n300,
                n200: state.n_katu,
                n100: state.n100,
                n50: state.n50,
                misses: state.misses,
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
                    TopOldOsuVersion::September22October24 => {
                        f.write_str("september 2022 - october 2024")
                    }
                    TopOldOsuVersion::October24Now => f.write_str("october 2024 - now"),
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
                    TopOldTaikoVersion::September22October24 => {
                        f.write_str("september 2022 - october 2024")
                    }
                    TopOldTaikoVersion::October24Now => f.write_str("october 2024 - now"),
                }
            }
            Self::Catch(version) => {
                f.write_str("Catch version ")?;

                match version {
                    TopOldCatchVersion::March14May20 => f.write_str("march 2014 - may 2020"),
                    TopOldCatchVersion::May20October24 => f.write_str("may 2020 - october 2024"),
                    TopOldCatchVersion::October24Now => f.write_str("october 2024 - now"),
                }
            }
            Self::Mania(version) => {
                f.write_str("Mania version ")?;

                match version {
                    TopOldManiaVersion::March14May18 => f.write_str("march 2014 - may 2018"),
                    TopOldManiaVersion::May18October22 => f.write_str("may 2018 - october 2022"),
                    TopOldManiaVersion::October22October24 => {
                        f.write_str("october 2022 - october 2024")
                    }
                    TopOldManiaVersion::October24Now => f.write_str("october 2024 - now"),
                }
            }
        }
    }
}

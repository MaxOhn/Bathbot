use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_pp::{
    catch::CatchScoreState, mania::ManiaScoreState, osu::OsuScoreState, taiko::TaikoScoreState,
    Beatmap,
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

    pub(super) fn generate_hitresults(
        self,
        map: &Beatmap,
        data: &SimulateData,
    ) -> Option<ScoreState> {
        match self {
            Self::Osu(_) => Some(Self::generate_hitresults_osu(map, data)),
            Self::Taiko(_) => Self::generate_hitresults_taiko(data),
            Self::Catch(_) => Self::generate_hitresults_catch(map, data),
            Self::Mania(_) => Some(Self::generate_hitresults_mania(map, data)),
        }
    }

    fn generate_hitresults_osu(map: &Beatmap, data: &SimulateData) -> ScoreState {
        let n_objects = map.hit_objects.len();

        let mut n300 = data.n300.unwrap_or(0) as usize;
        let mut n100 = data.n100.unwrap_or(0) as usize;
        let mut n50 = data.n50.unwrap_or(0) as usize;
        let n_misses = data.n_miss.unwrap_or(0) as usize;

        if let Some(acc) = data.acc {
            let acc = acc / 100.0;
            let target_total = (acc * (n_objects * 6) as f32).round() as usize;

            match (data.n300, data.n100, data.n50) {
                (Some(_), Some(_), Some(_)) => {
                    n300 += n_objects.saturating_sub(n300 + n100 + n50 + n_misses);
                }
                (Some(_), Some(_), None) => n50 = n_objects.saturating_sub(n300 + n100 + n_misses),
                (Some(_), None, Some(_)) => n100 = n_objects.saturating_sub(n300 + n50 + n_misses),
                (None, Some(_), Some(_)) => n300 = n_objects.saturating_sub(n100 + n50 + n_misses),
                (Some(_), None, None) => {
                    let delta = (target_total - n_objects.saturating_sub(n_misses))
                        .saturating_sub(n300 * 5);

                    n100 = delta % 5;
                    n50 = n_objects.saturating_sub(n300 + n100 + n_misses);

                    let curr_total = 6 * n300 + 2 * n100 + n50;

                    if curr_total < target_total {
                        let n = (target_total - curr_total).min(n50);
                        n50 -= n;
                        n100 += n;
                    } else {
                        let n = (curr_total - target_total).min(n100);
                        n100 -= n;
                        n50 += n;
                    }
                }
                (None, Some(_), None) => {
                    let delta =
                        (target_total - n_objects.saturating_sub(n_misses)).saturating_sub(n100);

                    n300 = delta / 5;

                    if n300 + n100 + n_misses > n_objects {
                        n300 -= (n300 + n100 + n_misses) - n_objects;
                    }

                    n50 = n_objects - n300 - n100 - n_misses;
                }
                (None, None, Some(_)) => {
                    let delta = target_total - n_objects.saturating_sub(n_misses);

                    n300 = delta / 5;
                    n100 = delta % 5;

                    if n300 + n100 + n50 + n_misses > n_objects {
                        let too_many = n300 + n100 + n50 + n_misses - n_objects;

                        if too_many > n100 {
                            n300 -= too_many - n100;
                            n100 = 0;
                        } else {
                            n100 -= too_many;
                        }
                    }

                    n100 += n_objects.saturating_sub(n300 + n100 + n50 + n_misses);

                    let curr_total = 6 * n300 + 2 * n100 + n50;

                    if curr_total < target_total {
                        let n = n100.min((target_total - curr_total) / 4);
                        n100 -= n;
                        n300 += n;
                    } else {
                        let n = n300.min((curr_total - target_total) / 4);
                        n300 -= n;
                        n100 += n;
                    }
                }
                (None, None, None) => {
                    let delta = target_total - n_objects.saturating_sub(n_misses);

                    n300 = delta / 5;
                    n100 = delta % 5;
                    n50 = n_objects.saturating_sub(n300 + n100 + n_misses);

                    // Shift n50 to n100 by sacrificing n300
                    let n = n300.min(n50 / 4);
                    n300 -= n;
                    n100 += 5 * n;
                    n50 -= 4 * n;
                }
            }
        } else {
            let remaining = n_objects.saturating_sub(n300 + n100 + n50 + n_misses);

            if data.n300.is_none() {
                n300 = remaining;
            } else if data.n100.is_none() {
                n100 = remaining;
            } else if data.n50.is_none() {
                n50 = remaining;
            } else {
                n300 += remaining;
            }
        }

        let state = OsuScoreState {
            n300,
            n100,
            n50,
            n_misses,
            max_combo: 0,
        };

        ScoreState::Osu(state)
    }

    fn generate_hitresults_taiko(data: &SimulateData) -> Option<ScoreState> {
        let total_result_count = data.max_combo as usize;

        let mut n300 = data.n300.unwrap_or(0) as usize;
        let mut n100 = data.n100.unwrap_or(0) as usize;
        let n_misses = data.n_miss.unwrap_or(0) as usize;

        if let Some(acc) = data.acc {
            let acc = acc / 100.0;

            match (data.n300, data.n100) {
                (Some(_), Some(_)) => {
                    n300 += total_result_count.saturating_sub(n300 + n100 + n_misses)
                }
                (Some(_), None) => n100 += total_result_count.saturating_sub(n300 + n_misses),
                (None, Some(_)) => n300 += total_result_count.saturating_sub(n100 + n_misses),
                (None, None) => {
                    let target_total = (acc * (total_result_count * 2) as f32).round() as usize;
                    n300 = target_total - (total_result_count.saturating_sub(n_misses));
                    n100 = total_result_count.saturating_sub(n300 + n_misses);
                }
            }
        } else {
            let remaining = total_result_count.saturating_sub(n300 + n100 + n_misses);

            match (data.n300, data.n100) {
                (Some(_), None) => n100 = remaining,
                (Some(_), Some(_)) => n300 += remaining,
                (None, _) => n300 = remaining,
            }
        }

        let state = TaikoScoreState {
            n300,
            n100,
            n_misses,
            max_combo: 0,
        };

        Some(ScoreState::Taiko(state))
    }

    // TODO: improve this
    fn generate_hitresults_catch(map: &Beatmap, data: &SimulateData) -> Option<ScoreState> {
        let attrs = rosu_pp::CatchStars::new(map).calculate();
        let max_combo = data.max_combo as usize;

        let mut n_fruits = data.n300.unwrap_or(0) as usize;
        let mut n_droplets = data.n100.unwrap_or(0) as usize;
        let mut n_tiny_droplets = data.n50.unwrap_or(0) as usize;
        let n_tiny_droplet_misses = data.n_katu.unwrap_or(0) as usize;
        let n_misses = data.n_miss.unwrap_or(0) as usize;

        let missing = max_combo
            .saturating_sub(n_fruits)
            .saturating_sub(n_droplets)
            .saturating_sub(n_misses);

        let missing_fruits = missing.saturating_sub(attrs.n_droplets.saturating_sub(n_droplets));

        n_fruits += missing_fruits;
        n_droplets += missing.saturating_sub(missing_fruits);

        n_tiny_droplets += attrs
            .n_tiny_droplets
            .saturating_sub(n_tiny_droplets)
            .saturating_sub(n_tiny_droplet_misses);

        let state = CatchScoreState {
            n_fruits,
            n_droplets,
            n_tiny_droplets,
            n_tiny_droplet_misses,
            n_misses,
            max_combo: 0,
        };

        Some(ScoreState::Catch(state))
    }

    fn generate_hitresults_mania(map: &Beatmap, data: &SimulateData) -> ScoreState {
        let n_objects = map.hit_objects.len();

        let mut n320 = data.n_geki.unwrap_or(0) as usize;
        let mut n300 = data.n300.unwrap_or(0) as usize;
        let mut n200 = data.n_katu.unwrap_or(0) as usize;
        let mut n100 = data.n100.unwrap_or(0) as usize;
        let mut n50 = data.n50.unwrap_or(0) as usize;
        let n_misses = data.n_miss.unwrap_or(0) as usize;

        if let Some(acc) = data.acc {
            let acc = acc / 100.0;
            let target_total = (acc * (n_objects * 6) as f32).round() as usize;

            match (data.n_geki, data.n300, data.n_katu, data.n100, data.n50) {
                (Some(_), Some(_), Some(_), Some(_), Some(_)) => {
                    let remaining =
                        n_objects.saturating_sub(n320 + n300 + n200 + n100 + n50 + n_misses);
                    n320 += remaining;
                }
                (Some(_), None, Some(_), Some(_), Some(_)) => {
                    n300 = n_objects.saturating_sub(n320 + n200 + n100 + n50 + n_misses)
                }
                (None, Some(_), Some(_), Some(_), Some(_)) => {
                    n320 = n_objects.saturating_sub(n300 + n200 + n100 + n50 + n_misses)
                }
                (Some(_), _, Some(_), Some(_), None) | (_, Some(_), Some(_), Some(_), None) => {
                    n50 = n_objects.saturating_sub(n320 + n300 + n200 + n100 + n_misses);
                }
                (Some(_), _, _, None, None) | (_, Some(_), _, None, None) => {
                    let n3x0 = n320 + n300;
                    let delta = (target_total - n_objects.saturating_sub(n_misses))
                        .saturating_sub(n3x0 * 5 + n200 * 3);

                    n100 = delta % 5;
                    n50 = n_objects.saturating_sub(n3x0 + n200 + n100 + n_misses);

                    let curr_total = 6 * n3x0 + 4 * n200 + 2 * n100 + n50;

                    if curr_total < target_total {
                        let n = (target_total - curr_total).min(n50);
                        n50 -= n;
                        n100 += n;
                    } else {
                        let n = (curr_total - target_total).min(n100);
                        n100 -= n;
                        n50 += n;
                    }
                }
                (Some(_), _, None, Some(_), None) | (_, Some(_), None, Some(_), None) => {
                    let n3x0 = n320 + n300;
                    let delta = (target_total - n_objects.saturating_sub(n_misses))
                        .saturating_sub(n3x0 * 5 + n100);

                    n200 = delta / 3;
                    n50 = n_objects.saturating_sub(n3x0 + n200 + n100 + n_misses);
                }
                (Some(_), _, None, None, Some(_)) | (_, Some(_), None, None, Some(_)) => {
                    n100 = n_objects.saturating_sub(n320 + n300 + n50 + n_misses);
                }
                (Some(_), _, None, Some(_), Some(_)) | (_, Some(_), None, Some(_), Some(_)) => {
                    n200 = n_objects.saturating_sub(n320 + n300 + n100 + n50 + n_misses);
                }
                (Some(_), _, Some(_), None, Some(_)) | (_, Some(_), Some(_), None, Some(_)) => {
                    n100 = n_objects.saturating_sub(n320 + n300 + n200 + n50 + n_misses);
                }
                (None, None, Some(_), Some(_), Some(_)) => {
                    n320 = n_objects.saturating_sub(n200 + n100 + n50 + n_misses);
                }
                (None, None, None, Some(_), Some(_)) => {
                    let delta =
                        (target_total - n_objects.saturating_sub(n_misses)).saturating_sub(n100);

                    n320 = delta / 5;
                    n200 = n_objects.saturating_sub(n320 + n100 + n50 + n_misses);

                    let curr_total = 6 * (n320 + n300) + 4 * n200 + 2 * n100 + n50;

                    if curr_total < target_total {
                        let n = n200.min((target_total - curr_total) / 2);
                        n200 -= n;
                        n320 += n;
                    } else {
                        let n = (n320 + n300).min((curr_total - target_total) / 2);
                        n200 += n;
                        n320 -= n;
                    }
                }
                (None, None, Some(_), None, None) => {
                    let delta = (target_total - n_objects.saturating_sub(n_misses))
                        .saturating_sub(n200 * 3);
                    n320 = delta / 5;

                    n100 = delta % 5;
                    n50 = n_objects.saturating_sub(n320 + n200 + n100 + n_misses);

                    let curr_total = 6 * (n320 + n300) + 4 * n200 * 2 * n100 + n50;

                    if curr_total < target_total {
                        let n = (target_total - curr_total).min(n50);
                        n50 -= n;
                        n100 += n;
                    } else {
                        let n = (curr_total - target_total).min(n100);
                        n100 -= n;
                        n50 += n;
                    }

                    // Shift n50 to n100
                    let n = n320.min(n50 / 4);

                    n320 -= n;
                    n100 += 5 * n;
                    n50 -= 4 * n;
                }
                (None, None, _, Some(_), None) => {
                    let delta = (target_total - n_objects.saturating_sub(n_misses))
                        .saturating_sub(n200 * 3 + n100);

                    n320 = delta / 5;

                    n50 = n_objects.saturating_sub(n320 + n300 + n200 + n100 + n_misses);
                }
                (None, None, _, None, Some(_)) => {
                    let delta =
                        target_total - n_objects.saturating_sub(n_misses).saturating_sub(n200 * 3);

                    n320 = delta / 5;

                    n100 = delta % 5;
                    n100 += n_objects.saturating_sub(n320 + n300 + n200 + n100 + n50 + n_misses);

                    let curr_total = 6 * (n320 + n300) + 4 * n200 + 2 * n100 + n50;

                    if curr_total < target_total {
                        let n = n100.min((target_total - curr_total) / 4);
                        n100 -= n;
                        n320 += n;
                    } else {
                        let n = (n320 + n300).min((curr_total - target_total) / 4);
                        n100 += n;
                        n320 -= n;
                    }
                }
                (None, None, None, None, None) => {
                    let delta = target_total - n_objects.saturating_sub(n_misses);

                    n320 = delta / 5;
                    n100 = delta % 5;
                    n50 = n_objects.saturating_sub(n320 + n300 + n100 + n_misses);

                    // Shift n50 to n100
                    let n = n320.min(n50 / 4);
                    n320 -= n;
                    n100 += 5 * n;
                    n50 -= 4 * n;
                }
            }
        } else {
            let remaining = n_objects.saturating_sub(n320 + n300 + n200 + n100 + n50 + n_misses);

            if data.n_geki.is_none() {
                n320 = remaining;
            } else if data.n300.is_none() {
                n300 = remaining;
            } else if data.n_katu.is_none() {
                n200 = remaining;
            } else if data.n100.is_none() {
                n100 = remaining;
            } else if data.n50.is_none() {
                n50 = remaining;
            } else {
                n320 += remaining;
            }
        }

        let state = ManiaScoreState {
            n320,
            n300,
            n200,
            n100,
            n50,
            n_misses,
        };

        ScoreState::Mania(state)
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

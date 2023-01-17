use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    numbers::{round, WithComma},
    osu::{calculate_grade, ModSelection},
    CowUtils, FooterBuilder,
};
use rosu_pp::{
    catch::CatchScoreState, mania::ManiaScoreState, osu::OsuScoreState, taiko::TaikoScoreState,
};
use rosu_v2::prelude::{GameMode, GameMods, Grade, ScoreStatistics};
use twilight_model::channel::embed::EmbedField;

use crate::{
    commands::osu::{TopOldCatchVersion, TopOldManiaVersion, TopOldOsuVersion, TopOldTaikoVersion},
    embeds::{ComboFormatter, HitResultFormatter, PpFormatter},
    manager::OsuMap,
    util::osu::{grade_completion_mods, MapInfo},
};

use super::KeyFormatter;

#[derive(EmbedData)]
pub struct SimulateEmbed {
    fields: Vec<EmbedField>,
    footer: FooterBuilder,
    image: String,
    title: String,
    url: String,
}

impl SimulateEmbed {
    pub fn new(map: &OsuMap, data: &SimulateData) -> Self {
        let mut title = format!(
            "{} - {} [{}]",
            map.artist().cow_escape_markdown(),
            map.title().cow_escape_markdown(),
            map.version().cow_escape_markdown(),
        );

        if matches!(data.version, TopOldVersion::Mania(_)) {
            let _ = write!(title, " {}", KeyFormatter::new(GameMods::NoMod, map));
        }

        let footer_text = format!(
            "{:?} map • {}",
            map.status(),
            VersionFormatter(data.version)
        );

        let footer =
            FooterBuilder::new(footer_text).icon_url(format!("{AVATAR_URL}{}", map.creator_id()));

        let image = map.cover().to_owned();
        let url = format!("{OSU_BASE}b/{}", map.map_id());

        let SimulateValues {
            stars,
            pp,
            max_pp,
            clock_rate,
            combo_ratio,
            score_state,
        } = data.simulate(map);

        let mods = data.mods.unwrap_or_default();
        let mut map_info = MapInfo::new(map, stars);

        let mut grade = if mods.intersects(GameMods::Hidden | GameMods::Flashlight) {
            Grade::XH
        } else {
            Grade::X
        };

        let (score, acc, hits) = match score_state {
            StateOrScore::Score(score) => {
                let score = EmbedField {
                    inline: true,
                    name: "Score".to_owned(),
                    value: WithComma::new(score).to_string(),
                };

                (Some(score), None, None)
            }
            StateOrScore::State(state) => {
                let (mode, stats) = state.into_parts();

                grade = calculate_grade(mode, mods, &stats);

                let acc = EmbedField {
                    inline: true,
                    name: "Acc".to_owned(),
                    value: format!("{}%", round(stats.accuracy(mode))),
                };

                let hits = EmbedField {
                    inline: true,
                    name: "Hits".to_owned(),
                    value: HitResultFormatter::new(mode, stats).to_string(),
                };

                (None, Some(acc), Some(hits))
            }
            StateOrScore::Neither => (None, None, None),
        };

        let (combo, ratio) = match combo_ratio {
            ComboOrRatio::Combo { score, max } => {
                let combo = EmbedField {
                    inline: true,
                    name: "Combo".to_owned(),
                    value: ComboFormatter::new(score, Some(max)).to_string(),
                };

                (Some(combo), None)
            }
            ComboOrRatio::Ratio(ratio) => {
                let ratio = EmbedField {
                    inline: true,
                    name: "Ratio".to_owned(),
                    value: ratio.to_string(),
                };

                (None, Some(ratio))
            }
            ComboOrRatio::Neither => (None, None),
        };

        let pp = EmbedField {
            inline: true,
            name: "PP".to_owned(),
            value: PpFormatter::new(Some(pp), Some(max_pp)).to_string(),
        };

        let grade = grade_completion_mods(mods, grade, map.n_objects() as u32, map);
        let mut fields = fields!["Grade", grade.into_owned(), true;];

        if let Some(acc) = acc {
            fields.push(acc);
        }

        if let Some(score) = score {
            fields.push(score);
        }

        if let Some(ratio) = ratio {
            fields.push(ratio);
        }

        if let Some(combo) = combo {
            fields.push(combo);
        }

        fields.push(pp);

        if let Some(clock_rate) = clock_rate {
            map_info.clock_rate(clock_rate);
            fields![fields { "Clock rate", format!("{clock_rate:.2}"), true }];
        }

        if let Some(hits) = hits {
            fields.push(hits);
        }

        map_info.mods(mods);
        fields![fields { "Map Info", map_info.to_string(), false; }];

        Self {
            fields,
            footer,
            image,
            title,
            url,
        }
    }
}

#[derive(Debug)]
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
    pub clock_rate: Option<f32>,
    pub version: TopOldVersion,
    pub ar: Option<f32>,
    pub cs: Option<f32>,
    pub hp: Option<f32>,
    pub od: Option<f32>,
}

macro_rules! setters {
    ( $( $fn:ident: $field:ident as $ty:ty; )* ) => {
        $(
            pub fn $fn(&mut self, val: $ty) {
                self.$field = Some(val);
            }
        )*
    }
}

impl SimulateData {
    pub fn set_mods(&mut self, mods: GameMods) {
        if ModSelection::Exact(mods).validate().is_ok() {
            self.mods = Some(mods);
        }
    }

    pub fn set_acc(&mut self, acc: f32) {
        self.acc = Some(acc.clamp(0.0, 100.0));
    }

    setters! {
        set_geki: n_geki as u32;
        set_katu: n_katu as u32;
        set_n300: n300 as u32;
        set_n100: n100 as u32;
        set_n50: n50 as u32;
        set_miss: n_miss as u32;
        set_combo: combo as u32;
        set_score: score as u32;
        set_clock_rate: clock_rate as f32;
    }

    fn simulate(&self, map: &OsuMap) -> SimulateValues {
        let mods = self.mods.unwrap_or_default().bits();

        macro_rules! simulate {
            (
                $( $calc:ident )::+ {
                    $( $calc_method:ident: $this_field:ident $( as $ty:ty )? ,)+
                }
            ) => {{
                let mut calc = $( $calc:: )* new(&map.pp_map).mods(mods);

                $(
                    if let Some(value) = self.$this_field {
                        calc = calc.$calc_method(value $( as $ty )?);
                    }
                )*

                let attrs = calc.calculate();

                let pp = attrs.pp;
                let stars = attrs.difficulty.stars;

                let max_pp = $( $calc:: )* new(&map.pp_map)
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
                    clock_rate: clock_rate as f64,
                    accuracy: acc as f64,
                }
            },
        };

        let state = self.version.generate_hitresults(map, self);

        let combo_ratio = match state {
            Some(ScoreState::Osu(_) | ScoreState::Taiko(_) | ScoreState::Catch(_)) => {
                match map.max_combo() {
                    Some(max) => ComboOrRatio::Combo {
                        score: self.combo.unwrap_or(max),
                        max,
                    },
                    None => ComboOrRatio::Neither,
                }
            }
            Some(ScoreState::Mania(ref state))
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
            Some(ScoreState::Mania(_)) | None => ComboOrRatio::Neither,
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
                self.mods.and_then(|mods| {
                    mods.intersects(GameMods::DoubleTime | GameMods::HalfTime)
                        .then(|| mods.clock_rate())
                })
            });

        let score_state = match state {
            Some(state @ (ScoreState::Osu(_) | ScoreState::Taiko(_) | ScoreState::Catch(_))) => {
                StateOrScore::State(state)
            }
            Some(state @ ScoreState::Mania(_))
                if self.version == TopOldVersion::Mania(TopOldManiaVersion::October22Now) =>
            {
                StateOrScore::State(state)
            }
            Some(ScoreState::Mania(_)) => match self.score {
                Some(score) => StateOrScore::Score(score),
                None => {
                    let mult = self
                        .mods
                        .unwrap_or_default()
                        .score_multiplier(GameMode::Mania);

                    StateOrScore::Score((1_000_000.0 * mult) as u32)
                }
            },
            None => match self.version {
                TopOldVersion::Mania(
                    TopOldManiaVersion::March14May18 | TopOldManiaVersion::May18October22,
                ) => match self.score {
                    Some(score) => StateOrScore::Score(score),
                    None => {
                        let mult = self
                            .mods
                            .unwrap_or_default()
                            .score_multiplier(GameMode::Mania);

                        StateOrScore::Score((1_000_000.0 * mult) as u32)
                    }
                },
                _ => StateOrScore::Neither,
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

struct SimulateValues {
    stars: f32,
    pp: f32,
    max_pp: f32,
    clock_rate: Option<f32>,
    combo_ratio: ComboOrRatio,
    score_state: StateOrScore,
}

enum StateOrScore {
    Score(u32),
    State(ScoreState),
    Neither,
}

enum ComboOrRatio {
    Combo { score: u32, max: u32 },
    Ratio(f32),
    Neither,
}

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
            "sim_osu_july21_november21" => Self::Osu(TopOldOsuVersion::July21November21),
            "sim_osu_january21_july21" => Self::Osu(TopOldOsuVersion::January21July21),
            "sim_osu_feburary19_january21" => Self::Osu(TopOldOsuVersion::February19January21),
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

    fn generate_hitresults(self, map: &OsuMap, data: &SimulateData) -> Option<ScoreState> {
        match self {
            TopOldVersion::Osu(_) => Some(Self::generate_hitresults_osu(map, data)),
            TopOldVersion::Taiko(_) => Self::generate_hitresults_taiko(map, data),
            TopOldVersion::Catch(_) => Self::generate_hitresults_catch(map, data),
            TopOldVersion::Mania(_) => Some(Self::generate_hitresults_mania(map, data)),
        }
    }

    fn generate_hitresults_osu(map: &OsuMap, data: &SimulateData) -> ScoreState {
        let n_objects = map.pp_map.hit_objects.len();

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

    fn generate_hitresults_taiko(map: &OsuMap, data: &SimulateData) -> Option<ScoreState> {
        let total_result_count = map.max_combo()? as usize;

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
    fn generate_hitresults_catch(map: &OsuMap, data: &SimulateData) -> Option<ScoreState> {
        let attrs = rosu_pp::CatchStars::new(&map.pp_map).calculate();
        let max_combo = map.max_combo()? as usize;

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

    fn generate_hitresults_mania(map: &OsuMap, data: &SimulateData) -> ScoreState {
        let n_objects = map.pp_map.hit_objects.len();

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

enum ScoreState {
    Osu(OsuScoreState),
    Taiko(TaikoScoreState),
    Catch(CatchScoreState),
    Mania(ManiaScoreState),
}

impl ScoreState {
    fn into_parts(self) -> (GameMode, ScoreStatistics) {
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

struct VersionFormatter(TopOldVersion);

impl Display for VersionFormatter {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.0 {
            TopOldVersion::Osu(version) => {
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
            TopOldVersion::Taiko(version) => {
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
            TopOldVersion::Catch(version) => {
                f.write_str("Catch version ")?;

                match version {
                    TopOldCatchVersion::March14May20 => f.write_str("march 2014 - may 2020"),
                    TopOldCatchVersion::May20Now => f.write_str("may 2020 - now"),
                }
            }
            TopOldVersion::Mania(version) => {
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

use std::{borrow::Cow, fmt::Write, sync::Arc};

use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    fields,
    modal::{ModalBuilder, TextInputBuilder},
    numbers::{round, WithComma},
    osu::calculate_grade,
    CowUtils, EmbedBuilder, FooterBuilder,
};
use eyre::{ContextCompat, Report, Result};
use futures::future::BoxFuture;
use rosu_v2::{
    mods,
    prelude::{GameModsIntermode, Grade},
};
use twilight_model::{
    channel::message::{embed::EmbedField, Component},
    id::{marker::UserMarker, Id},
};

pub use self::{attrs::SimulateAttributes, data::SimulateData, top_old::TopOldVersion};
use crate::{
    active::{
        impls::simulate::data::{ComboOrRatio, SimulateValues, StateOrScore},
        BuildPage, ComponentResult, IActiveMessage,
    },
    core::Context,
    embeds::{ComboFormatter, HitResultFormatter, KeyFormatter, PpFormatter},
    manager::OsuMap,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::{grade_completion_mods, MapInfo},
        Authored, ComponentExt, ModalExt,
    },
};

mod attrs;
mod data;
mod state;
mod top_old;

pub struct SimulateComponents {
    map: OsuMap,
    data: SimulateData,
    msg_owner: Id<UserMarker>,
}

impl IActiveMessage for SimulateComponents {
    fn build_page<'a>(&'a mut self, _: Arc<Context>) -> BoxFuture<'a, Result<BuildPage>> {
        if let Some(ar) = self.data.attrs.ar {
            self.map.pp_map.ar = ar;
        }

        if let Some(cs) = self.data.attrs.cs {
            self.map.pp_map.cs = cs;
        }

        if let Some(hp) = self.data.attrs.hp {
            self.map.pp_map.hp = hp;
        }

        if let Some(od) = self.data.attrs.od {
            self.map.pp_map.od = od;
        }

        let mut title = format!(
            "{} - {} [{}]",
            self.map.artist().cow_escape_markdown(),
            self.map.title().cow_escape_markdown(),
            self.map.version().cow_escape_markdown(),
        );

        if matches!(self.data.version, TopOldVersion::Mania(_)) {
            let _ = write!(title, " {}", KeyFormatter::new(&mods!(Mania), &self.map));
        }

        let footer_text = format!(
            "{:?} mapset of {} • {}",
            self.map.status(),
            self.map.creator(),
            self.data.version,
        );

        let footer = FooterBuilder::new(footer_text)
            .icon_url(format!("{AVATAR_URL}{}", self.map.creator_id()));

        let image = self.map.cover().to_owned();
        let url = format!("{OSU_BASE}b/{}", self.map.map_id());

        let SimulateValues {
            stars,
            pp,
            max_pp,
            clock_rate,
            combo_ratio,
            score_state,
        } = self.data.simulate(&self.map);

        let mods = self
            .data
            .mods
            .as_ref()
            .map(Cow::Borrowed)
            .unwrap_or_default();

        let mut map_info = MapInfo::new(&self.map, stars);

        let mut grade = if mods.contains_any(mods!(HD FL)) {
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

                grade = calculate_grade(mode, &mods, &stats);

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

        let grade = grade_completion_mods(&mods, grade, self.map.n_objects() as u32, &self.map);
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

        fields![fields { "PP", PpFormatter::new(Some(pp), Some(max_pp)).to_string(), true; }];

        if let Some(clock_rate) = clock_rate {
            map_info.clock_rate(clock_rate);
            fields![fields { "Clock rate", format!("{clock_rate:.2}"), true }];
        }

        if let Some(hits) = hits {
            fields.push(hits);
        }

        map_info.mods(mods.bits());
        fields![fields { "Map Info", map_info.to_string(), false; }];

        let embed = EmbedBuilder::new()
            .fields(fields)
            .footer(footer)
            .image(image)
            .title(title)
            .url(url);

        let content = "Simulated score:";

        BuildPage::new(embed, true).content(content).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.data.version.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err).boxed(),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore.boxed();
        }

        let modal = match component.data.custom_id.as_str() {
            "sim_mods" => {
                let input = TextInputBuilder::new("sim_mods", "Mods")
                    .placeholder("E.g. hd or HdHRdteZ")
                    .required(false);

                ModalBuilder::new("sim_mods", "Specify mods").input(input)
            }
            "sim_combo" => {
                let input = TextInputBuilder::new("sim_combo", "Combo")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_combo", "Specify combo").input(input)
            }
            "sim_acc" => {
                let input = TextInputBuilder::new("sim_acc", "Accuracy")
                    .placeholder("Number")
                    .required(false);

                ModalBuilder::new("sim_acc", "Specify accuracy").input(input)
            }
            "sim_geki" => {
                let input = TextInputBuilder::new("sim_geki", "Amount of gekis")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_geki", "Specify the amount of gekis").input(input)
            }
            "sim_katu" => {
                let input = TextInputBuilder::new("sim_katu", "Amount of katus")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_katu", "Specify the amount of katus").input(input)
            }
            "sim_n300" => {
                let input = TextInputBuilder::new("sim_n300", "Amount of 300s")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_n300", "Specify the amount of 300s").input(input)
            }
            "sim_n100" => {
                let input = TextInputBuilder::new("sim_n100", "Amount of 100s")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_n100", "Specify the amount of 100s").input(input)
            }
            "sim_n50" => {
                let input = TextInputBuilder::new("sim_n50", "Amount of 50s")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_n50", "Specify the amount of 50s").input(input)
            }
            "sim_miss" => {
                let input = TextInputBuilder::new("sim_miss", "Amount of misses")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_miss", "Specify the amount of misses").input(input)
            }
            "sim_score" => {
                let input = TextInputBuilder::new("sim_score", "Score")
                    .placeholder("Integer")
                    .required(false);

                ModalBuilder::new("sim_score", "Specify the score").input(input)
            }
            "sim_clock_rate" => {
                let clock_rate = TextInputBuilder::new("sim_clock_rate", "Clock rate")
                    .placeholder("Specify a clock rate")
                    .required(false);

                let bpm = TextInputBuilder::new(
                    "sim_bpm",
                    "BPM (overwritten if clock rate is specified)",
                )
                .placeholder("Specify a BPM")
                .required(false);

                ModalBuilder::new("sim_speed_adjustments", "Speed adjustments")
                    .input(clock_rate)
                    .input(bpm)
            }
            "sim_attrs" => {
                let ar = TextInputBuilder::new("sim_ar", "AR")
                    .placeholder("Specify an approach rate")
                    .required(false);

                let cs = TextInputBuilder::new("sim_cs", "CS")
                    .placeholder("Specify a circle size")
                    .required(false);

                let hp = TextInputBuilder::new("sim_hp", "HP")
                    .placeholder("Specify a drain rate")
                    .required(false);

                let od = TextInputBuilder::new("sim_od", "OD")
                    .placeholder("Specify an overall difficulty")
                    .required(false);

                ModalBuilder::new("sim_attrs", "Attributes")
                    .input(ar)
                    .input(cs)
                    .input(hp)
                    .input(od)
            }
            "sim_osu_version" | "sim_taiko_version" | "sim_catch_version" | "sim_mania_version" => {
                return Box::pin(self.handle_topold_menu(ctx, component));
            }
            other => {
                warn!(name = %other, ?component, "Unknown simulate component");

                return ComponentResult::Ignore.boxed();
            }
        };

        ComponentResult::CreateModal(modal).boxed()
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(self.async_handle_modal(ctx, modal))
    }
}

impl SimulateComponents {
    pub fn new(map: OsuMap, data: SimulateData, msg_owner: Id<UserMarker>) -> Self {
        Self {
            map,
            data,
            msg_owner,
        }
    }

    async fn handle_topold_menu(
        &mut self,
        ctx: &Context,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let Some(version) = component.data.values.first() else {
            return ComponentResult::Err(eyre!("Missing simulate version"));
        };

        let Some(version) = TopOldVersion::from_menu_str(version) else {
            return ComponentResult::Err(eyre!("Unknown TopOldVersion `{version}`"));
        };

        if let Err(err) = component.defer(ctx).await.map_err(Report::new) {
            return ComponentResult::Err(err.wrap_err("Failed to defer component"));
        }

        self.data.version = version;

        ComponentResult::BuildPage
    }

    async fn async_handle_modal(
        &mut self,
        ctx: &Context,
        modal: &mut InteractionModal,
    ) -> Result<()> {
        if modal.user_id()? != self.msg_owner {
            return Ok(());
        }

        let input = modal
            .data
            .components
            .first()
            .and_then(|row| row.components.first())
            .wrap_err("Missing simulate modal input")?
            .value
            .as_deref()
            .filter(|val| !val.is_empty());

        match modal.data.custom_id.as_str() {
            "sim_mods" => {
                let mods_res = input.map(|s| {
                    s.trim_start_matches('+')
                        .trim_end_matches('!')
                        .parse::<GameModsIntermode>()
                });

                let mods = match mods_res {
                    Some(Ok(value)) => Some(value),
                    Some(Err(_)) => {
                        debug!(input, "Failed to parse simulate mods");

                        return Ok(());
                    }
                    None => None,
                };

                match mods.map(|mods| mods.with_mode(self.map.mode())) {
                    Some(Some(mods)) if mods.is_valid() => self.data.mods = Some(mods),
                    None => self.data.mods = None,
                    Some(Some(mods)) => {
                        debug!("Incompatible mods {mods}");

                        return Ok(());
                    }
                    Some(None) => {
                        debug!(input, "Invalid mods for mode");

                        return Ok(());
                    }
                }
            }
            "sim_acc" => match input.map(str::parse::<f32>) {
                Some(Ok(value)) => self.data.acc = Some(value.clamp(0.0, 100.0)),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate accuracy");

                    return Ok(());
                }
                None => self.data.acc = None,
            },
            "sim_combo" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.combo = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate combo");

                    return Ok(());
                }
                None => self.data.combo = None,
            },
            "sim_geki" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.n_geki = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate gekis");

                    return Ok(());
                }
                None => self.data.n_geki = None,
            },
            "sim_katu" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.n_katu = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate katus");

                    return Ok(());
                }
                None => self.data.n_katu = None,
            },
            "sim_n300" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.n300 = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate 300s");

                    return Ok(());
                }
                None => self.data.n300 = None,
            },
            "sim_n100" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.n100 = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate 100s");

                    return Ok(());
                }
                None => self.data.n100 = None,
            },
            "sim_n50" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.n50 = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate 50s");

                    return Ok(());
                }
                None => self.data.n50 = None,
            },
            "sim_miss" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.n_miss = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate misses");

                    return Ok(());
                }
                None => self.data.n_miss = None,
            },
            "sim_score" => match input.map(str::parse) {
                Some(Ok(value)) => self.data.score = Some(value),
                Some(Err(_)) => {
                    debug!(input, "Failed to parse simulate score");

                    return Ok(());
                }
                None => self.data.score = None,
            },
            "sim_speed_adjustments" => {
                self.data.clock_rate = parse_attr(&*modal, "sim_clock_rate");
                self.data.bpm = parse_attr(&*modal, "sim_bpm");
            }
            "sim_attrs" => {
                self.data.attrs.ar = parse_attr(&modal, "sim_ar");
                self.data.attrs.cs = parse_attr(&modal, "sim_cs");
                self.data.attrs.hp = parse_attr(&modal, "sim_hp");
                self.data.attrs.od = parse_attr(&modal, "sim_od");
            }
            other => warn!(name = %other, ?modal, "Unknown simulate modal"),
        }

        if let Err(err) = modal.defer(ctx).await {
            warn!(?err, "Failed to defer modal");
        }

        Ok(())
    }
}

fn parse_attr(modal: &InteractionModal, component_id: &str) -> Option<f32> {
    modal
        .data
        .components
        .iter()
        .find_map(|row| {
            row.components.first().and_then(|component| {
                (component.custom_id == component_id).then(|| {
                    component
                        .value
                        .as_deref()
                        .filter(|value| !value.is_empty())
                        .map(str::parse)
                        .and_then(Result::ok)
                })
            })
        })
        .flatten()
}

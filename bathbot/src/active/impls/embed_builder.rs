use std::{
    cmp::{self, Ordering},
    fmt::Write,
    future::ready,
};

use bathbot_model::{command_fields::ScoreEmbedSettings, rosu_v2::user::User};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    datetime::{HowLongAgoDynamic, SecToMinSec},
    numbers::round,
    MessageBuilder,
};
use eyre::{Result, WrapErr};
use futures::future::BoxFuture;
use rosu_pp::model::beatmap::BeatmapAttributes;
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption},
        Component, ReactionType,
    },
    id::{marker::UserMarker, Id},
};

use super::{SingleScoreContent, SingleScorePagination};
use crate::{
    active::{response::ActiveResponse, BuildPage, ComponentResult, IActiveMessage},
    commands::utility::{ScoreEmbedData, ScoreEmbedDataWrap},
    core::Context,
    embeds::{ComboFormatter, HitResultFormatter, ModsFormatter},
    manager::redis::RedisData,
    util::{
        interaction::InteractionComponent,
        osu::{GradeFormatter, ScoreFormatter},
        Authored, Emote,
    },
};

pub struct ScoreEmbedBuilderActive {
    inner: SingleScorePagination,
    content: ContentStatus,
    section: EmbedSection,
    value_kind: ValueKind,
    msg_owner: Id<UserMarker>,
}

impl ScoreEmbedBuilderActive {
    pub fn new(
        user: &RedisData<User>,
        data: ScoreEmbedDataWrap,
        settings: ScoreEmbedSettings,
        score_data: ScoreData,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        let inner = SingleScorePagination::new(
            user,
            Box::from([data]),
            settings,
            score_data,
            msg_owner,
            SingleScoreContent::None,
        );

        Self {
            inner,
            content: ContentStatus::Preview,
            section: EmbedSection::None,
            value_kind: ValueKind::None,
            msg_owner,
        }
    }

    async fn async_handle_component(
        &mut self,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore;
        }

        match component.data.custom_id.as_str() {
            "embed_builder_section" => {
                let Some(value) = component.data.values.first() else {
                    return ComponentResult::Err(eyre!(
                        "Missing value for builder component `{}`",
                        component.data.custom_id
                    ));
                };

                self.section = match value.as_str() {
                    "score_data" => EmbedSection::ScoreData,
                    "image" => EmbedSection::Image,
                    "button" => EmbedSection::Buttons,
                    _ => {
                        return ComponentResult::Err(eyre!(
                            "Invalid value `{value}` for builder component `{}`",
                            component.data.custom_id
                        ));
                    }
                };
            }
            "embed_builder_value" => {
                let Some(value) = component.data.values.first() else {
                    return ComponentResult::Err(eyre!(
                        "Missing value for builder component `{}`",
                        component.data.custom_id
                    ));
                };

                self.value_kind = match value.as_str() {
                    "grade" => ValueKind::Grade,
                    "mods" => ValueKind::Mods,
                    "score" => ValueKind::Score,
                    "acc" => ValueKind::Accuracy,
                    "score_date" => ValueKind::ScoreDate,
                    "pp" => ValueKind::Pp,
                    "combo" => ValueKind::Combo,
                    "hitresults" => ValueKind::Hitresults,
                    "len" => ValueKind::Length,
                    "bpm" => ValueKind::Bpm,
                    "ar" => ValueKind::Ar,
                    "cs" => ValueKind::Cs,
                    "hp" => ValueKind::Hp,
                    "od" => ValueKind::Od,
                    "n_objects" => ValueKind::CountObjects,
                    "n_spinners" => ValueKind::CountSpinners,
                    _ => {
                        return ComponentResult::Err(eyre!(
                            "Invalid value `{value}` for builder component `{}`",
                            component.data.custom_id
                        ));
                    }
                };
            }
            "embed_builder_show_button" => {
                let last_y = self
                    .inner
                    .new_settings
                    .values
                    .last()
                    .expect("at least one field")
                    .y;

                let value = SettingValue {
                    inner: self.value_kind.into(),
                    y: last_y + 1,
                };

                self.inner.new_settings.values.push(value);
            }
            "embed_builder_hide_button" => {
                let Some(idx) = self
                    .inner
                    .new_settings
                    .values
                    .iter()
                    .position(|value| value.kind() == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot remove value that's not present"));
                };

                if self.inner.new_settings.disable_hide(idx) {
                    return ComponentResult::Err(eyre!("Conditions were not met to hide value"));
                }

                let curr_y = self.inner.new_settings.values[idx].y;

                let curr_x = self.inner.new_settings.values[..idx]
                    .iter()
                    .rev()
                    .take_while(|value| value.y == curr_y)
                    .count();

                let next_y = self
                    .inner
                    .new_settings
                    .values
                    .get(idx + 1)
                    .map(|value| value.y);

                if curr_x == 0 && next_y.is_some_and(|next_y| next_y != curr_y) {
                    for value in self.inner.new_settings.values[idx + 1..].iter_mut() {
                        value.y -= 1;
                    }
                }

                self.inner.new_settings.values.remove(idx);
            }
            "embed_builder_value_left" => {
                let Some(idx) = self
                    .inner
                    .new_settings
                    .values
                    .iter()
                    .position(|value| value.kind() == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                self.inner.new_settings.values.swap(idx - 1, idx);
            }
            "embed_builder_value_up" => {
                let Some(idx) = self
                    .inner
                    .new_settings
                    .values
                    .iter()
                    .position(|value| value.kind() == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                let can_move = match self.inner.new_settings.values.get(idx) {
                    Some(value) if value.y == 1 => {
                        let count_non_first_row = self
                            .inner
                            .new_settings
                            .values
                            .iter()
                            .skip_while(|value| value.y == 0)
                            .count();

                        count_non_first_row > 1
                    }
                    Some(_) => true,
                    None => false,
                };

                if !can_move {
                    return ComponentResult::Err(eyre!("Conditions were not met to move value up"));
                }

                let curr_y = self.inner.new_settings.values[idx].y;

                let mut prev_iter = self.inner.new_settings.values[..idx].iter().rev();

                let curr_x = prev_iter
                    .by_ref()
                    .take_while(|value| value.y == curr_y)
                    .count();

                let prev_row_len = prev_iter.take_while(|value| value.y == curr_y - 1).count();

                let to_right_count = self.inner.new_settings.values[idx + 1..]
                    .iter()
                    .take_while(|value| value.y == curr_y)
                    .count();

                self.inner.new_settings.values[idx].y -= 1;

                if curr_x == 0 && to_right_count == 0 {
                    for value in self.inner.new_settings.values[idx + 1..].iter_mut() {
                        value.y -= 1;
                    }
                }

                let shift = match prev_row_len.cmp(&curr_x) {
                    Ordering::Less | Ordering::Equal => curr_x,
                    Ordering::Greater => prev_row_len,
                };

                self.inner.new_settings.values[idx - shift..=idx].rotate_right(1);
            }
            "embed_builder_value_down" => {
                let Some(idx) = self
                    .inner
                    .new_settings
                    .values
                    .iter()
                    .position(|value| value.kind() == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                let curr_y = self.inner.new_settings.values[idx].y;

                let curr_x = self.inner.new_settings.values[..idx]
                    .iter()
                    .rev()
                    .take_while(|value| value.y == curr_y)
                    .count();

                let mut next_iter = self.inner.new_settings.values[idx + 1..].iter();

                let to_right_count = next_iter
                    .by_ref()
                    .take_while(|value| value.y == curr_y)
                    .count();

                let next_row_len = next_iter.take_while(|value| value.y == curr_y + 1).count();

                if curr_x == 0 && to_right_count == 0 {
                    for value in self.inner.new_settings.values[idx + 1..].iter_mut() {
                        value.y -= 1;
                    }
                } else {
                    self.inner.new_settings.values[idx].y += 1;
                }

                let shift = 1 + to_right_count + cmp::min(next_row_len, curr_x);
                self.inner.new_settings.values[idx..idx + shift].rotate_left(1);
            }
            "embed_builder_value_right" => {
                let Some(idx) = self
                    .inner
                    .new_settings
                    .values
                    .iter()
                    .position(|value| value.kind() == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                let curr_y = self.inner.new_settings.values[idx].y;
                let next = self.inner.new_settings.values.get(idx + 1);

                if next.is_some_and(|next| next.y == curr_y) {
                    self.inner.new_settings.values.swap(idx, idx + 1);
                } else {
                    return ComponentResult::Err(eyre!(
                        "Cannot move right-most value to the right"
                    ));
                }
            }
            "embed_builder_pp" => {
                let mut max = false;
                let mut if_fc = false;

                for value in component.data.values.iter() {
                    match value.as_str() {
                        "max" => max = true,
                        "if_fc" => if_fc = true,
                        _ => {
                            return ComponentResult::Err(eyre!(
                                "Unknown value `{value}` for builder component {}",
                                component.data.custom_id
                            ))
                        }
                    }
                }

                if let Some(value) = self
                    .inner
                    .new_settings
                    .values
                    .iter_mut()
                    .find(|value| value.kind() == ValueKind::Pp)
                {
                    value.inner = Value::Pp(PpValue { max, if_fc });
                }
            }
            "embed_builder_combo" => {
                let mut max = false;

                for value in component.data.values.iter() {
                    match value.as_str() {
                        "max" => max = true,
                        _ => {
                            return ComponentResult::Err(eyre!(
                                "Unknown value `{value}` for builder component {}",
                                component.data.custom_id
                            ))
                        }
                    }
                }

                if let Some(value) = self
                    .inner
                    .new_settings
                    .values
                    .iter_mut()
                    .find(|value| value.kind() == ValueKind::Combo)
                {
                    value.inner = Value::Combo(ComboValue { max });
                }
            }
            "embed_builder_hitresults_full" => {
                if let Some(value) = self
                    .inner
                    .new_settings
                    .values
                    .iter_mut()
                    .find(|value| value.kind() == ValueKind::Hitresults)
                {
                    value.inner = Value::Hitresults(HitresultsValue::Full);
                }
            }
            "embed_builder_hitresults_misses" => {
                if let Some(value) = self
                    .inner
                    .new_settings
                    .values
                    .iter_mut()
                    .find(|value| value.kind() == ValueKind::Hitresults)
                {
                    value.inner = Value::Hitresults(HitresultsValue::OnlyMisses);
                }
            }
            "embed_builder_bpm_emote" => {
                if let Some(value) = self
                    .inner
                    .new_settings
                    .values
                    .iter_mut()
                    .find(|value| value.kind() == ValueKind::Bpm)
                {
                    value.inner = Value::Bpm(BpmValue::Emote);
                }
            }
            "embed_builder_bpm_text" => {
                if let Some(value) = self
                    .inner
                    .new_settings
                    .values
                    .iter_mut()
                    .find(|value| value.kind() == ValueKind::Bpm)
                {
                    value.inner = Value::Bpm(BpmValue::Text);
                }
            }
            "embed_builder_image" => {
                let Some(value) = component.data.values.first() else {
                    return ComponentResult::Err(eyre!(
                        "Missing value for builder component {}",
                        component.data.custom_id
                    ));
                };

                self.inner.new_settings.image = match value.as_str() {
                    "thumbnail" => SettingsImage::Thumbnail,
                    "image" => SettingsImage::Image,
                    "image_strains" => SettingsImage::ImageWithStrains,
                    "hide" => SettingsImage::Hide,
                    _ => {
                        return ComponentResult::Err(eyre!(
                            "Unknown value `{value}` for builder component {}",
                            component.data.custom_id
                        ))
                    }
                }
            }
            "embed_builder_buttons" => {
                let mut pagination = false;
                let mut render = false;
                let mut miss_analyzer = false;

                for value in component.data.values.iter() {
                    match value.as_str() {
                        "pagination" => pagination = true,
                        "render" => render = true,
                        "miss_analyzer" => miss_analyzer = true,
                        _ => {
                            return ComponentResult::Err(eyre!(
                                "Unknown value `{value}` for builder component {}",
                                component.data.custom_id
                            ))
                        }
                    }
                }

                self.inner.new_settings.buttons = SettingsButtons {
                    pagination,
                    render,
                    miss_analyzer,
                };
            }
            other => {
                warn!(name = %other, ?component, "Unknown score embed builder component");

                return ComponentResult::Ignore;
            }
        }

        let store_fut =
            Context::user_config().store_score_embed_settings(self.msg_owner, &self.inner.settings);

        match store_fut.await {
            Ok(_) => self.content = ContentStatus::Preview,
            Err(err) => {
                self.content = ContentStatus::Error;
                warn!(?err);
            }
        }

        ComponentResult::BuildPage
    }
}

impl IActiveMessage for ScoreEmbedBuilderActive {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let content = Box::from(self.content.as_str());

        let mark_idx = self
            .inner
            .new_settings
            .values
            .iter()
            .position(|value| value.kind() == self.value_kind);

        Box::pin(self.inner.async_build_page(content, mark_idx))
    }

    fn build_components(&self) -> Vec<Component> {
        macro_rules! section_option {
            ( $label:literal, $value:literal, $variant:ident ) => {
                SelectMenuOption {
                    default: matches!(self.section, EmbedSection::$variant),
                    description: None,
                    emoji: None,
                    label: $label.to_owned(),
                    value: $value.to_owned(),
                }
            };
        }

        macro_rules! kind_option {
            ( $label:literal, $value:literal, $variant:ident ) => {
                SelectMenuOption {
                    default: matches!(self.value_kind, ValueKind::$variant),
                    description: None,
                    emoji: None,
                    label: $label.to_owned(),
                    value: $value.to_owned(),
                }
            };
        }

        let mut components = vec![Component::ActionRow(ActionRow {
            components: vec![Component::SelectMenu(SelectMenu {
                custom_id: "embed_builder_section".to_owned(),
                disabled: false,
                max_values: None,
                min_values: None,
                options: vec![
                    section_option!("Score data", "score_data", ScoreData),
                    section_option!("Image", "image", Image),
                    section_option!("Buttons", "buttons", Buttons),
                ],
                placeholder: Some("Choose an embed section".to_owned()),
            })],
        })];

        match self.section {
            EmbedSection::None => {}
            EmbedSection::ScoreData => {
                components.push(Component::ActionRow(ActionRow {
                    components: vec![Component::SelectMenu(SelectMenu {
                        custom_id: "embed_builder_value".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options: vec![
                            kind_option!("Grade", "grade", Grade),
                            kind_option!("Mods", "mods", Mods),
                            kind_option!("Score", "score", Score),
                            kind_option!("Accuracy", "acc", Accuracy),
                            kind_option!("Score date", "score_date", ScoreDate),
                            kind_option!("PP", "pp", Pp),
                            kind_option!("Combo", "combo", Combo),
                            kind_option!("Hitresults", "hitresults", Hitresults),
                            kind_option!("Length", "len", Length),
                            kind_option!("BPM", "bpm", Bpm),
                            kind_option!("Approach Rate", "ar", Ar),
                            kind_option!("Circle Size", "cs", Cs),
                            kind_option!("Drain rate", "hp", Hp),
                            kind_option!("Overall Difficulty", "od", Od),
                            kind_option!("Count objects", "n_objects", CountObjects),
                            kind_option!("Count spinners", "n_spinners", CountSpinners),
                        ],
                        placeholder: Some("Select a value to display".to_owned()),
                    })],
                }));

                let show_hide_row = |idx: Option<usize>| {
                    let disable_hide = match idx {
                        Some(idx) => self.inner.new_settings.disable_hide(idx),
                        None => true,
                    };

                    Component::ActionRow(ActionRow {
                        components: vec![
                            Component::Button(Button {
                                custom_id: Some("embed_builder_show_button".to_owned()),
                                disabled: idx.is_some(),
                                emoji: None,
                                label: Some("Show".to_owned()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_hide_button".to_owned()),
                                disabled: disable_hide,
                                emoji: None,
                                label: Some("Hide".to_owned()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                        ],
                    })
                };

                // TODO: consider field name length
                let arrow_row = |idx: Option<usize>| {
                    let (disable_left, disable_up, disable_down, disable_right) =
                        if let Some(idx) = idx {
                            let curr_y = self.inner.new_settings.values[idx].y;

                            let to_left = self.inner.new_settings.values[..idx]
                                .iter()
                                .rev()
                                .take_while(|value| value.y == curr_y)
                                .count();

                            let to_right = self.inner.new_settings.values[idx + 1..]
                                .iter()
                                .take_while(|value| value.y == curr_y)
                                .count();

                            let is_last_row =
                                self.inner.new_settings.values[idx + to_right + 1..].is_empty();

                            let disable_up = match curr_y {
                                0 => true,
                                1 => {
                                    let count_non_first_row = self
                                        .inner
                                        .new_settings
                                        .values
                                        .iter()
                                        .skip_while(|value| value.y == 0)
                                        .count();

                                    count_non_first_row <= 1
                                }
                                _ => false,
                            };

                            // No need to check if the first row only contains
                            // one value because if so then the current second
                            // row would be moved up anyway.
                            let disable_down = is_last_row && to_left == 0 && to_right == 0;

                            (to_left == 0, disable_up, disable_down, to_right == 0)
                        } else {
                            (false, false, false, false)
                        };

                    Component::ActionRow(ActionRow {
                        components: vec![
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_left".to_owned()),
                                disabled: disable_left,
                                emoji: Some(ReactionType::Unicode {
                                    name: "◀️".to_owned(),
                                }),
                                label: Some("Left".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_up".to_owned()),
                                disabled: disable_up,
                                emoji: Some(ReactionType::Unicode {
                                    name: "🔼".to_owned(),
                                }),
                                label: Some("Up".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_down".to_owned()),
                                disabled: disable_down,
                                emoji: Some(ReactionType::Unicode {
                                    name: "🔽".to_owned(),
                                }),
                                label: Some("Down".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_right".to_owned()),
                                disabled: disable_right,
                                emoji: Some(ReactionType::Unicode {
                                    name: "▶️".to_owned(),
                                }),
                                label: Some("Right".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                        ],
                    })
                };

                let idx = self
                    .inner
                    .new_settings
                    .values
                    .iter()
                    .position(|value| value.kind() == self.value_kind);

                match self.value_kind {
                    ValueKind::None => {}
                    ValueKind::Grade => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Mods => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Score => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Accuracy => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::ScoreDate => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Pp => {
                        components.push(show_hide_row(idx));

                        let pp = match idx
                            .and_then(|idx| self.inner.new_settings.values.get(idx))
                            .map(|value| &value.inner)
                        {
                            Some(Value::Pp(pp)) => *pp,
                            None => Default::default(),
                            Some(_) => unreachable!(),
                        };

                        let pp_options = vec![
                            SelectMenuOption {
                                default: pp.max,
                                description: None,
                                emoji: None,
                                label: "Show max pp".to_owned(),
                                value: "max".to_owned(),
                            },
                            SelectMenuOption {
                                default: pp.if_fc,
                                description: None,
                                emoji: None,
                                label: "Show if-FC pp".to_owned(),
                                value: "if_fc".to_owned(),
                            },
                        ];

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![Component::SelectMenu(SelectMenu {
                                custom_id: "embed_builder_pp".to_owned(),
                                disabled: idx.is_none(),
                                max_values: Some(pp_options.len() as u8),
                                min_values: Some(0),
                                options: pp_options,
                                placeholder: Some("Only show score pp".to_owned()),
                            })],
                        }));

                        components.push(arrow_row(idx));
                    }
                    ValueKind::Combo => {
                        components.push(show_hide_row(idx));

                        let combo = match idx
                            .and_then(|idx| self.inner.new_settings.values.get(idx))
                            .map(|value| &value.inner)
                        {
                            Some(Value::Combo(combo)) => *combo,
                            None => Default::default(),
                            Some(_) => unreachable!(),
                        };

                        let combo_options = vec![SelectMenuOption {
                            default: combo.max,
                            description: None,
                            emoji: None,
                            label: "Show max combo".to_owned(),
                            value: "max".to_owned(),
                        }];

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![Component::SelectMenu(SelectMenu {
                                custom_id: "embed_builder_combo".to_owned(),
                                disabled: idx.is_none(),
                                max_values: Some(combo_options.len() as u8),
                                min_values: Some(0),
                                options: combo_options,
                                placeholder: Some("Only show score combo".to_owned()),
                            })],
                        }));

                        components.push(arrow_row(idx));
                    }
                    ValueKind::Hitresults => {
                        components.push(show_hide_row(idx));

                        let hitresults = idx
                            .and_then(|idx| self.inner.new_settings.values.get(idx))
                            .and_then(|value| match value.inner {
                                Value::Hitresults(ref hitresults) => Some(hitresults),
                                _ => None,
                            });

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_hitresults_full".to_owned()),
                                    disabled: matches!(
                                        hitresults,
                                        Some(HitresultsValue::Full) | None
                                    ),
                                    emoji: None,
                                    label: Some("Full".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_hitresults_misses".to_owned()),
                                    disabled: matches!(
                                        hitresults,
                                        Some(HitresultsValue::OnlyMisses) | None
                                    ),
                                    emoji: None,
                                    label: Some("Only misses".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                            ],
                        }));

                        components.push(arrow_row(idx));
                    }
                    ValueKind::Length => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Bpm => {
                        components.push(show_hide_row(idx));

                        let bpm = idx
                            .and_then(|idx| self.inner.new_settings.values.get(idx))
                            .and_then(|value| match value.inner {
                                Value::Bpm(ref bpm) => Some(bpm),
                                _ => None,
                            });

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_bpm_emote".to_owned()),
                                    disabled: matches!(bpm, Some(BpmValue::Emote) | None),
                                    emoji: Some(Emote::Bpm.reaction_type()),
                                    label: Some("Emote".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_bpm_text".to_owned()),
                                    disabled: matches!(bpm, Some(BpmValue::Text) | None),
                                    emoji: None,
                                    label: Some("Text".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                            ],
                        }));

                        components.push(arrow_row(idx));
                    }
                    ValueKind::Ar => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Cs => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Hp => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Od => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::CountObjects => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::CountSpinners => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                }
            }
            EmbedSection::Image => {
                let options = vec![
                    SelectMenuOption {
                        default: self.inner.new_settings.image == SettingsImage::Thumbnail,
                        description: None,
                        emoji: None,
                        label: "Thumbnail".to_owned(),
                        value: "thumbnail".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.new_settings.image == SettingsImage::Image,
                        description: None,
                        emoji: None,
                        label: "Image".to_owned(),
                        value: "image".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.new_settings.image == SettingsImage::ImageWithStrains,
                        description: None,
                        emoji: None,
                        label: "Image with strains".to_owned(),
                        value: "image_strains".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.new_settings.image == SettingsImage::Hide,
                        description: None,
                        emoji: None,
                        label: "Hide".to_owned(),
                        value: "hide".to_owned(),
                    },
                ];

                components.push(Component::ActionRow(ActionRow {
                    components: vec![Component::SelectMenu(SelectMenu {
                        custom_id: "embed_builder_image".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options,
                        placeholder: None,
                    })],
                }));
            }
            EmbedSection::Buttons => {
                let options = vec![
                    SelectMenuOption {
                        default: self.inner.new_settings.buttons.pagination,
                        description: None,
                        emoji: None,
                        label: "Pagination".to_owned(),
                        value: "pagination".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.new_settings.buttons.render,
                        description: None,
                        emoji: None,
                        label: "Render".to_owned(),
                        value: "render".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.new_settings.buttons.miss_analyzer,
                        description: None,
                        emoji: None,
                        label: "Miss analyzer".to_owned(),
                        value: "miss_analyzer".to_owned(),
                    },
                ];

                components.push(Component::ActionRow(ActionRow {
                    components: vec![Component::SelectMenu(SelectMenu {
                        custom_id: "embed_builder_buttons".to_owned(),
                        disabled: false,
                        max_values: Some(options.len() as u8),
                        min_values: Some(0),
                        options,
                        placeholder: Some("Hide buttons".to_owned()),
                    })],
                }));
            }
        }

        components
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        Box::pin(self.async_handle_component(component))
    }

    fn on_timeout(&mut self, response: ActiveResponse) -> BoxFuture<'_, Result<()>> {
        let content = match self.content {
            ContentStatus::Preview => "Settings saved successfully ✅",
            content @ ContentStatus::Error => content.as_str(),
        };

        let builder = MessageBuilder::new()
            .content(content)
            .components(Vec::new());

        match response.update(builder) {
            Some(update_fut) => {
                let fut = async {
                    update_fut
                        .await
                        .map(|_| ())
                        .wrap_err("Failed to remove components")
                };
                Box::pin(fut)
            }
            None => Box::pin(ready(Err(eyre!(
                "Lacking permission to update message on timeout"
            )))),
        }
    }
}

#[derive(Copy, Clone)]
enum ContentStatus {
    Preview,
    Error,
}

impl ContentStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "Embed preview:",
            Self::Error => "⚠️ Something went wrong while saving settings",
        }
    }
}

#[derive(Copy, Clone)]
pub enum EmbedSection {
    None,
    ScoreData,
    Image,
    Buttons,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ValueKind {
    None,
    Grade,
    Mods,
    Score,
    Accuracy,
    ScoreDate,
    Pp,
    Combo,
    Hitresults,
    // Ratio,
    Length,
    Bpm,
    Ar,
    Cs,
    Hp,
    Od,
    CountObjects,
    CountSpinners,
    // MapRankedDate,
    // Mapper,
}

#[derive(Copy, Clone, Debug)]
pub struct PpValue {
    pub max: bool,
    pub if_fc: bool,
}

impl Default for PpValue {
    fn default() -> Self {
        Self {
            max: true,
            if_fc: true,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ComboValue {
    pub max: bool,
}

impl Default for ComboValue {
    fn default() -> Self {
        Self { max: true }
    }
}

#[derive(Debug, Default)]
pub enum HitresultsValue {
    Full,
    #[default]
    OnlyMisses,
}

#[derive(Debug, Default)]
pub enum BpmValue {
    #[default]
    Emote,
    Text,
}

#[derive(Debug)]
pub struct Settings {
    pub values: Vec<SettingValue>,
    pub image: SettingsImage,
    pub buttons: SettingsButtons,
}

impl Settings {
    fn disable_hide(&self, idx: usize) -> bool {
        match self.values.get(idx) {
            Some(value) => match value.y {
                // disable hide button if first row has only one value
                0 => self.values.get(1).map_or(true, |value| value.y != 0),
                // disable hide button if there's only one value not in the
                // first row
                1 => self.values[idx..].len() == 1,
                _ => false,
            },
            None => true,
        }
    }

    pub fn write_field(
        &self,
        data: &ScoreEmbedData,
        score_data: ScoreData,
        mark_idx: Option<usize>,
    ) -> (String, String) {
        const SEP_NAME: &str = "\t";
        const SEP_VALUE: &str = " • ";

        let map_attrs = data.map.attributes().mods(data.score.mods.clone()).build();

        let mut field_name = String::new();
        let mut field_value = String::new();

        let mut writer = &mut field_name;

        let curr = self.values.first().expect("at least one field");

        let next = self
            .values
            .get(1)
            .filter(|next| next.y == 0)
            .map(|value| &value.inner);

        if mark_idx == Some(0) {
            writer.push_str("__");
        }

        match (&curr.inner, next) {
            (
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
                Some(Value::Ar | Value::Cs | Value::Hp | Value::Od),
            ) => {
                let _ = match curr.inner {
                    Value::Ar => write!(writer, "`AR: {}", round(map_attrs.ar as f32)),
                    Value::Cs => write!(writer, "`CS: {}", round(map_attrs.cs as f32)),
                    Value::Hp => write!(writer, "`HP: {}", round(map_attrs.hp as f32)),
                    Value::Od => write!(writer, "`OD: {}", round(map_attrs.od as f32)),
                    _ => unreachable!(),
                };
            }
            _ => {
                Self::write_field_inner(curr, data, &map_attrs, score_data, writer);
            }
        }

        if mark_idx == Some(0) {
            writer.push_str("__");
        }

        for (i, window) in self.values.windows(3).enumerate() {
            let [prev, curr, next] = window else {
                unreachable!()
            };

            if mark_idx == Some(i) {
                writer.push_str("__");
            }

            match (&prev.inner, &curr.inner, &next.inner) {
                (Value::Grade, Value::Mods, _) if prev.y == curr.y => {
                    let _ = write!(writer, " {}", ModsFormatter::new(&data.score.mods));
                }
                (
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                ) if prev.y == curr.y && curr.y == next.y => {
                    let _ = match curr.inner {
                        Value::Ar => write!(writer, "AR: {} ", round(map_attrs.ar as f32)),
                        Value::Cs => write!(writer, "CS: {} ", round(map_attrs.cs as f32)),
                        Value::Hp => write!(writer, "HP: {} ", round(map_attrs.hp as f32)),
                        Value::Od => write!(writer, "OD: {} ", round(map_attrs.od as f32)),
                        _ => unreachable!(),
                    };
                }
                (
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    _,
                ) if prev.y == curr.y => {
                    let _ = match curr.inner {
                        Value::Ar => write!(writer, "AR: {}`", round(map_attrs.ar as f32)),
                        Value::Cs => write!(writer, "CS: {}`", round(map_attrs.cs as f32)),
                        Value::Hp => write!(writer, "HP: {}`", round(map_attrs.hp as f32)),
                        Value::Od => write!(writer, "OD: {}`", round(map_attrs.od as f32)),
                        _ => unreachable!(),
                    };
                }
                (
                    _,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                ) if curr.y == next.y => {
                    let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };

                    let _ = match curr.inner {
                        Value::Ar => {
                            write!(writer, "{sep}`AR: {} ", round(map_attrs.ar as f32))
                        }
                        Value::Cs => {
                            write!(writer, "{sep}`CS: {} ", round(map_attrs.cs as f32))
                        }
                        Value::Hp => {
                            write!(writer, "{sep}`HP: {} ", round(map_attrs.hp as f32))
                        }
                        Value::Od => {
                            write!(writer, "{sep}`OD: {} ", round(map_attrs.od as f32))
                        }
                        _ => unreachable!(),
                    };
                }
                _ => {
                    if prev.y == curr.y {
                        let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };
                        writer.push_str(sep);
                    } else if prev.y == 0 {
                        writer = &mut field_value;
                    } else {
                        writer.push('\n');
                    }

                    Self::write_field_inner(curr, data, &map_attrs, score_data, writer);
                }
            }

            if mark_idx == Some(i) {
                writer.push_str("__");
            }
        }

        let Some([prev, last]) = self.values.get(self.values.len() - 2..) else {
            unreachable!("at least two values");
        };

        if mark_idx == Some(self.values.len() - 1) {
            writer.push_str("__");
        }

        if prev.y != last.y {
            if prev.y == 0 {
                writer = &mut field_value;
            } else {
                writer.push('\n');
            }

            Self::write_field_inner(last, data, &map_attrs, score_data, writer);
        } else {
            match (&prev.inner, &last.inner) {
                (Value::Grade, Value::Mods) => {
                    let _ = write!(writer, " {}", ModsFormatter::new(&data.score.mods));
                }
                (
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                ) => {
                    let _ = match last.inner {
                        Value::Ar => write!(writer, "AR: {}`", round(map_attrs.ar as f32)),
                        Value::Cs => write!(writer, "CS: {}`", round(map_attrs.cs as f32)),
                        Value::Hp => write!(writer, "HP: {}`", round(map_attrs.hp as f32)),
                        Value::Od => write!(writer, "OD: {}`", round(map_attrs.od as f32)),
                        _ => unreachable!(),
                    };
                }
                _ => {
                    let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };
                    writer.push_str(sep);
                    Self::write_field_inner(last, data, &map_attrs, score_data, writer);
                }
            }
        }

        if mark_idx == Some(self.values.len() - 1) {
            writer.push_str("__");
        }

        (field_name, field_value)
    }

    fn write_field_inner(
        value: &SettingValue,
        data: &ScoreEmbedData,
        map_attrs: &BeatmapAttributes,
        score_data: ScoreData,
        writer: &mut String,
    ) {
        match &value.inner {
            Value::Grade if value.y == 0 => {
                // TODO: Fail percent
                let _ = write!(
                    writer,
                    "{}",
                    GradeFormatter::new(data.score.grade, None, false),
                );
            }
            Value::Grade => {
                // TODO: Fail percent
                let _ = write!(
                    writer,
                    "{}",
                    GradeFormatter::new(data.score.grade, data.score.legacy_id, true),
                );
            }
            Value::Mods => {
                let _ = write!(writer, "{}", data.score.mods);
            }
            Value::Score => {
                let _ = write!(writer, "{}", ScoreFormatter::new(&data.score, score_data));
            }
            Value::Accuracy => {
                let _ = write!(writer, "{}%", round(data.score.accuracy));
            }
            Value::ScoreDate => {
                // TODO: different format in footer
                let _ = write!(writer, "{}", HowLongAgoDynamic::new(&data.score.ended_at));
            }
            Value::Pp(pp) => {
                let _ = write!(writer, "**{:.2}", data.score.pp);

                match (pp.max, data.if_fc_pp.filter(|_| pp.if_fc)) {
                    (true, Some(if_fc_pp)) => {
                        let _ = write!(
                            writer,
                            "**/{max:.2}PP ~~({if_fc_pp:.2}pp)~~",
                            max = data.max_pp.max(data.score.pp)
                        );
                    }
                    (true, None) => {
                        let _ = write!(writer, "**/{:.2}PP", data.max_pp.max(data.score.pp));
                    }
                    (false, Some(if_fc_pp)) => {
                        let _ = write!(writer, "pp** ~~({if_fc_pp:.2}pp)~~");
                    }
                    (false, None) => writer.push_str("pp**"),
                }
            }
            Value::Combo(combo) => {
                let score_combo = data.score.max_combo;

                let _ = if combo.max {
                    write!(
                        writer,
                        "{}",
                        ComboFormatter::new(score_combo, Some(data.max_combo))
                    )
                } else {
                    write!(writer, "**{score_combo}x**")
                };
            }
            Value::Hitresults(hitresults) => {
                let _ = match hitresults {
                    HitresultsValue::Full => write!(
                        writer,
                        "{}",
                        HitResultFormatter::new(data.score.mode, data.score.statistics.clone())
                    ),
                    HitresultsValue::OnlyMisses => {
                        write!(
                            writer,
                            "{}{}",
                            data.score.statistics.count_miss,
                            Emote::Miss
                        )
                    }
                };
            }
            Value::Length => {
                let clock_rate = map_attrs.clock_rate as f32;
                let seconds_drain = (data.map.seconds_drain() as f32 / clock_rate) as u32;

                let _ = write!(writer, "`{}`", SecToMinSec::new(seconds_drain).pad_secs());
            }
            Value::Bpm(bpm) => {
                let clock_rate = map_attrs.clock_rate as f32;
                let value = round(data.map.bpm() * clock_rate);

                let _ = match bpm {
                    BpmValue::Emote => write!(writer, "{} **{value}**", Emote::Bpm),
                    BpmValue::Text => write!(writer, "**{value} BPM**"),
                };
            }
            Value::Ar => {
                let _ = write!(writer, "`AR: {}`", round(map_attrs.ar as f32));
            }
            Value::Cs => {
                let _ = write!(writer, "`CS: {}`", round(map_attrs.cs as f32));
            }
            Value::Hp => {
                let _ = write!(writer, "`HP: {}`", round(map_attrs.hp as f32));
            }
            Value::Od => {
                let _ = write!(writer, "`OD: {}`", round(map_attrs.od as f32));
            }
            Value::CountObjects => {
                let _ = write!(writer, "{} {}", Emote::CountObjects, data.map.n_objects());
            }
            Value::CountSpinners => {
                let _ = write!(writer, "{} {}", Emote::CountSpinners, data.map.n_spinners());
            }
        }
    }
}

#[derive(Debug)]
pub struct SettingValue {
    pub inner: Value,
    pub y: i32,
}

impl SettingValue {
    fn kind(&self) -> ValueKind {
        match self.inner {
            Value::Grade => ValueKind::Grade,
            Value::Mods => ValueKind::Mods,
            Value::Score => ValueKind::Score,
            Value::Accuracy => ValueKind::Accuracy,
            Value::Pp(_) => ValueKind::Pp,
            Value::ScoreDate => ValueKind::ScoreDate,
            Value::Combo(_) => ValueKind::Combo,
            Value::Hitresults(_) => ValueKind::Hitresults,
            Value::Length => ValueKind::Length,
            Value::Bpm(_) => ValueKind::Bpm,
            Value::Ar => ValueKind::Ar,
            Value::Cs => ValueKind::Cs,
            Value::Hp => ValueKind::Hp,
            Value::Od => ValueKind::Od,
            Value::CountObjects => ValueKind::CountObjects,
            Value::CountSpinners => ValueKind::CountSpinners,
        }
    }
}

#[derive(Debug)]
pub enum Value {
    Grade,
    Mods,
    Score,
    Accuracy,
    ScoreDate,
    Pp(PpValue),
    Combo(ComboValue),
    Hitresults(HitresultsValue),
    // Ratio,
    Length,
    Bpm(BpmValue),
    Ar,
    Cs,
    Hp,
    Od,
    CountObjects,
    CountSpinners,
    // MapRankedDate,
    // Mapper,
}

impl From<ValueKind> for Value {
    fn from(kind: ValueKind) -> Self {
        match kind {
            ValueKind::Grade => Self::Grade,
            ValueKind::Mods => Self::Mods,
            ValueKind::Score => Self::Score,
            ValueKind::Accuracy => Self::Accuracy,
            ValueKind::ScoreDate => Self::ScoreDate,
            ValueKind::Pp => Self::Pp(Default::default()),
            ValueKind::Combo => Self::Combo(Default::default()),
            ValueKind::Hitresults => Self::Hitresults(Default::default()),
            ValueKind::Length => Self::Length,
            ValueKind::Bpm => Self::Bpm(Default::default()),
            ValueKind::Ar => Self::Ar,
            ValueKind::Cs => Self::Cs,
            ValueKind::Hp => Self::Hp,
            ValueKind::Od => Self::Od,
            ValueKind::CountObjects => Self::CountObjects,
            ValueKind::CountSpinners => Self::CountSpinners,
            ValueKind::None => unreachable!(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            values: vec![
                SettingValue {
                    inner: Value::Grade,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Mods,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Score,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Accuracy,
                    y: 0,
                },
                SettingValue {
                    inner: Value::ScoreDate,
                    y: 0,
                },
                SettingValue {
                    inner: Value::Pp(Default::default()),
                    y: 1,
                },
                SettingValue {
                    inner: Value::Combo(Default::default()),
                    y: 1,
                },
                SettingValue {
                    inner: Value::Hitresults(Default::default()),
                    y: 1,
                },
                SettingValue {
                    inner: Value::Length,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Cs,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Ar,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Od,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Hp,
                    y: 2,
                },
                SettingValue {
                    inner: Value::Bpm(Default::default()),
                    y: 2,
                },
                // SettingValue {
                //     inner: Value::Mapper,
                //     y: 3,
                // },
                // SettingValue {
                //     inner: Value::MapRankedDate,
                //     y: 3,
                // },
            ],
            image: SettingsImage::Thumbnail,
            buttons: SettingsButtons {
                pagination: true,
                render: true,
                miss_analyzer: true,
            },
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SettingsImage {
    Thumbnail,
    Image,
    ImageWithStrains,
    Hide,
}

#[derive(Debug)]
pub struct SettingsButtons {
    pub pagination: bool,
    pub render: bool,
    pub miss_analyzer: bool,
}

/*
v Single-Menu
+-- [Score data]
|   v Single-Menu
|   +-- [Grade]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Mods]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Score]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Accuracy]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Score date]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [PP]
|   |   - Hide/Show | Inline/Field buttons
|   |   v Multi-Menu
|   |   +-- [Max PP]
|   |   +-- [If-FC PP]
|   |   - Left/Up/Down/Right buttons
|   +-- [Combo]
|   |   - Hide/Show | Inline/Field buttons
|   |   v Multi-Menu
|   |   +-- [Max combo]
|   |   - Left/Up/Down/Right buttons
|   +-- [Hitresults]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Full/Only misses buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Ratio]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Length]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [BPM]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Emote/Text buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [AR]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [CS]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [HP]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [OD]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Count objects]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Count sliders]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Count spinners]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Map ranked date]
|   |   - Hide/Show | Inline/Field buttons
|   |   - Left/Up/Down/Right buttons
|   +-- [Mapper]
|       - Hide/Show | Inline/Field buttons
|       - Left/Up/Down/Right buttons
+-- [Image]
|   v Single-Menu
|   +-- [Image]
|   +-- [Image with map strains]
|   +-- [Thumbnail]
|   +-- [Hide]
+-- [Buttons]
    v Multi-Menu
    +-- [Pagination]
    +-- [Render]
    +-- [Miss analyzer]
*/

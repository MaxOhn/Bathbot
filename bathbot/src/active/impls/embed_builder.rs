use std::{
    borrow::Cow,
    cmp::{self, Ordering},
    fmt::Write,
    future::ready,
    time::Duration,
};

use bathbot_model::{command_fields::ScoreEmbedSettings, rosu_v2::user::User};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    datetime::{HowLongAgoDynamic, HowLongAgoText, SecToMinSec, SHORT_NAIVE_DATETIME_FORMAT},
    numbers::round,
    MessageBuilder,
};
use eyre::{Result, WrapErr};
use futures::future::BoxFuture;
use rosu_pp::model::beatmap::BeatmapAttributes;
use rosu_v2::{
    model::{GameMode, Grade},
    prelude::RankStatus,
};
use time::OffsetDateTime;
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
    embeds::{ComboFormatter, HitResultFormatter},
    manager::redis::RedisData,
    util::{
        interaction::InteractionComponent,
        osu::{GradeFormatter, ScoreFormatter},
        Authored, Emote,
    },
};

const FOOTER_Y: u8 = u8::MAX;
const DAY: Duration = Duration::from_secs(60 * 60 * 24);

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
                    "ranked_date" => ValueKind::MapRankedDate,
                    "mapper" => ValueKind::Mapper,
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
            "embed_builder_reset_button" => {
                self.inner.new_settings.values = Settings::default().values;
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
                    Some(value) => value.y > 0,
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

                let (prev_y, prev_row_len) = prev_iter.next().map_or((0, 0), |prev| {
                    let len = prev_iter.take_while(|value| value.y == prev.y).count() + 1;

                    (prev.y, len)
                });

                let to_right_count = self.inner.new_settings.values[idx + 1..]
                    .iter()
                    .take_while(|value| value.y == curr_y)
                    .count();

                if self.inner.new_settings.values[idx].y == FOOTER_Y {
                    self.inner.new_settings.values[idx].y = prev_y;
                } else {
                    self.inner.new_settings.values[idx].y -= 1;
                }

                if curr_x == 0 && to_right_count == 0 {
                    for value in self.inner.new_settings.values[idx + 1..].iter_mut() {
                        if value.y == FOOTER_Y {
                            break;
                        }

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
                        if value.y == FOOTER_Y {
                            break;
                        }

                        value.y -= 1;
                    }

                    if next_row_len == 0 {
                        self.inner.new_settings.values[idx].y = FOOTER_Y;
                    }
                } else {
                    self.inner.new_settings.values[idx].y += 1;
                }

                let shift_next_line = if next_row_len == 0 {
                    0
                } else {
                    // Footer row len
                    self.inner.new_settings.values[idx + to_right_count + 1..]
                        .iter()
                        .count()
                };

                let shift = 1 + to_right_count + cmp::min(shift_next_line, curr_x);
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
            "embed_builder_mapper" => {
                let mut with_status = false;

                for value in component.data.values.iter() {
                    match value.as_str() {
                        "status" => with_status = true,
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
                    .find(|value| value.kind() == ValueKind::Mapper)
                {
                    value.inner = Value::Mapper(MapperValue { with_status });
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
                            kind_option!("AR", "ar", Ar),
                            kind_option!("CS", "cs", Cs),
                            kind_option!("HP", "hp", Hp),
                            kind_option!("OD", "od", Od),
                            kind_option!("Count objects", "n_objects", CountObjects),
                            kind_option!("Count spinners", "n_spinners", CountSpinners),
                            SelectMenuOption {
                                default: matches!(self.value_kind, ValueKind::MapRankedDate),
                                description: Some(
                                    "Note: This value only shows on ranked maps".to_owned(),
                                ),
                                emoji: None,
                                label: "Map ranked date".to_owned(),
                                value: "ranked_date".to_owned(),
                            },
                            kind_option!("Mapper", "mapper", Mapper),
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
                            Component::Button(Button {
                                custom_id: Some("embed_builder_reset_button".to_owned()),
                                disabled: false,
                                emoji: None,
                                label: Some("Reset all".to_owned()),
                                style: ButtonStyle::Danger,
                                url: None,
                            }),
                        ],
                    })
                };

                // TODO: consider field name length or limit row size
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

                            let disable_up = curr_y == 0;

                            // No need to check if the first row only contains
                            // one value because if so then the current second
                            // row would be moved up anyway.
                            let disable_down = is_last_row && to_left == 0 && to_right == 0;

                            (to_left == 0, disable_up, disable_down, to_right == 0)
                        } else {
                            (true, true, true, true)
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
                    ValueKind::MapRankedDate => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Mapper => {
                        components.push(show_hide_row(idx));

                        let mapper = match idx
                            .and_then(|idx| self.inner.new_settings.values.get(idx))
                            .map(|value| &value.inner)
                        {
                            Some(Value::Mapper(mapper)) => *mapper,
                            None => Default::default(),
                            Some(_) => unreachable!(),
                        };

                        let mapper_options = vec![SelectMenuOption {
                            default: mapper.with_status,
                            description: Some(
                                "Note: Won't show on ranked maps if `Map ranked date` enabled"
                                    .to_owned(),
                            ),
                            emoji: None,
                            label: "Include mapset status".to_owned(),
                            value: "status".to_owned(),
                        }];

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![Component::SelectMenu(SelectMenu {
                                custom_id: "embed_builder_mapper".to_owned(),
                                disabled: idx.is_none(),
                                max_values: Some(mapper_options.len() as u8),
                                min_values: Some(0),
                                options: mapper_options,
                                placeholder: Some("Hide mapset status".to_owned()),
                            })],
                        }));

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
    Length,
    Bpm,
    Ar,
    Cs,
    Hp,
    Od,
    CountObjects,
    CountSpinners,
    MapRankedDate,
    Mapper,
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

#[derive(Copy, Clone, Debug, Default)]
pub enum HitresultsValue {
    Full,
    #[default]
    OnlyMisses,
}

#[derive(Copy, Clone, Debug, Default)]
pub enum BpmValue {
    #[default]
    Emote,
    Text,
}

#[derive(Copy, Clone, Debug)]
pub struct MapperValue {
    pub with_status: bool,
}

impl Default for MapperValue {
    fn default() -> Self {
        Self { with_status: true }
    }
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

    pub fn apply(
        &self,
        data: &ScoreEmbedData,
        score_data: ScoreData,
        mark_idx: Option<usize>,
    ) -> (String, String, Option<String>) {
        const SEP_NAME: &str = "\t";
        const SEP_VALUE: &str = " • ";

        let map_attrs = data.map.attributes().mods(data.score.mods.clone()).build();

        let mut field_name = String::new();
        let mut field_value = String::new();
        let mut footer_text = String::new();

        let mut writer = &mut field_name;

        let first = self.values.first().expect("at least one field");

        let next = self
            .values
            .get(1)
            .filter(|next| next.y == 0)
            .map(|value| &value.inner);

        match (&first.inner, next) {
            (
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
                Some(Value::Ar | Value::Cs | Value::Hp | Value::Od),
            ) => {
                writer.push('`');

                if mark_idx == Some(0) {
                    writer.push_str("*");
                }

                let _ = match first.inner {
                    Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                    Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                    Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                    Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                    _ => unreachable!(),
                };

                if mark_idx == Some(0) {
                    writer.push_str("*");
                }
            }
            (Value::MapRankedDate, _) if data.map.ranked_date().is_none() => {}
            _ => {
                let mut value = Cow::Borrowed(first);

                if matches!(&first.inner, Value::Mapper(mapper) if mapper.with_status)
                    && data.map.status() == RankStatus::Ranked
                    && data.map.ranked_date().is_some()
                    && self
                        .values
                        .iter()
                        .any(|value| value.kind() == ValueKind::MapRankedDate)
                {
                    value = Cow::Owned(SettingValue {
                        inner: Value::Mapper(MapperValue { with_status: false }),
                        y: first.y,
                    });
                }

                if mark_idx == Some(0) {
                    writer.push_str("__");
                }

                Self::write_field_inner(&value, data, &map_attrs, score_data, writer);

                if mark_idx == Some(0) {
                    writer.push_str("__");
                }
            }
        }

        for (window, i) in self.values.windows(3).zip(1..) {
            let [prev, curr, next] = window else {
                unreachable!()
            };

            match (&prev.inner, &curr.inner, &next.inner) {
                (Value::Grade, Value::Mods, _) if prev.y == curr.y => {
                    writer.push(' ');

                    if mark_idx == Some(i) {
                        writer.push_str("__");
                    }

                    let _ = write!(writer, "+{}", data.score.mods);

                    if mark_idx == Some(i) {
                        writer.push_str("__");
                    }
                }
                (
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                ) if prev.y == curr.y && curr.y == next.y => {
                    if mark_idx == Some(i) {
                        writer.push_str("*");
                    }

                    let _ = match curr.inner {
                        Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                        Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                        Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                        Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                        _ => unreachable!(),
                    };

                    if mark_idx == Some(i) {
                        writer.push_str("*");
                    }

                    writer.push(' ');
                }
                (
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    _,
                ) if prev.y == curr.y => {
                    if mark_idx == Some(i) {
                        writer.push_str("*");
                    }

                    let _ = match curr.inner {
                        Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                        Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                        Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                        Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                        _ => unreachable!(),
                    };

                    if mark_idx == Some(i) {
                        writer.push_str("*");
                    }

                    writer.push('`');
                }
                (
                    _,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                ) if curr.y == next.y => {
                    let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };
                    writer.push_str(sep);
                    writer.push('`');

                    if mark_idx == Some(i) {
                        writer.push_str("*");
                    }

                    let _ = match curr.inner {
                        Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                        Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                        Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                        Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                        _ => unreachable!(),
                    };

                    if mark_idx == Some(i) {
                        writer.push_str("*");
                    }

                    writer.push(' ');
                }
                (_, Value::MapRankedDate, _) if data.map.ranked_date().is_none() => {}
                _ => {
                    let mut value = Cow::Borrowed(curr);

                    if matches!(&curr.inner, Value::Mapper(mapper) if mapper.with_status)
                        && data.map.status() == RankStatus::Ranked
                        && data.map.ranked_date().is_some()
                        && self
                            .values
                            .iter()
                            .any(|value| value.kind() == ValueKind::MapRankedDate)
                    {
                        value = Cow::Owned(SettingValue {
                            inner: Value::Mapper(MapperValue { with_status: false }),
                            y: curr.y,
                        });
                    }

                    if prev.y == curr.y {
                        let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };
                        writer.push_str(sep);
                    } else if curr.y == FOOTER_Y {
                        writer = &mut footer_text;
                    } else if prev.y == 0 {
                        writer = &mut field_value;
                    } else {
                        writer.push('\n');
                    }

                    let mark = if value.y == FOOTER_Y { "*" } else { "__" };

                    if mark_idx == Some(i) {
                        writer.push_str(mark);
                    }

                    Self::write_field_inner(&value, data, &map_attrs, score_data, writer);

                    if mark_idx == Some(i) {
                        writer.push_str(mark);
                    }
                }
            }
        }

        let Some([prev, last]) = self.values.get(self.values.len() - 2..) else {
            unreachable!("at least two values");
        };

        if !(last.kind() == ValueKind::MapRankedDate && data.map.ranked_date().is_none()) {
            let last_idx = self.values.len() - 1;
            let mark = if last.y == FOOTER_Y { "*" } else { "__" };

            if prev.y != last.y {
                if last.y == FOOTER_Y {
                    writer = &mut footer_text;
                } else if prev.y == 0 {
                    writer = &mut field_value;
                } else {
                    writer.push('\n');
                }

                let mut value = Cow::Borrowed(last);

                if matches!(&last.inner, Value::Mapper(mapper) if mapper.with_status)
                    && data.map.status() == RankStatus::Ranked
                    && data.map.ranked_date().is_some()
                    && self
                        .values
                        .iter()
                        .any(|value| value.kind() == ValueKind::MapRankedDate)
                {
                    value = Cow::Owned(SettingValue {
                        inner: Value::Mapper(MapperValue { with_status: false }),
                        y: last.y,
                    });
                }

                if mark_idx == Some(last_idx) {
                    writer.push_str(mark);
                }

                Self::write_field_inner(&value, data, &map_attrs, score_data, writer);

                if mark_idx == Some(last_idx) {
                    writer.push_str(mark);
                }
            } else {
                match (&prev.inner, &last.inner) {
                    (Value::Grade, Value::Mods) => {
                        writer.push(' ');

                        if mark_idx == Some(last_idx) {
                            writer.push_str("__");
                        }

                        let _ = write!(writer, "+{}", data.score.mods);

                        if mark_idx == Some(last_idx) {
                            writer.push_str("__");
                        }
                    }
                    (
                        Value::Ar | Value::Cs | Value::Hp | Value::Od,
                        Value::Ar | Value::Cs | Value::Hp | Value::Od,
                    ) => {
                        if mark_idx == Some(last_idx) {
                            writer.push_str("*");
                        }

                        let _ = match last.inner {
                            Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                            Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                            Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                            Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                            _ => unreachable!(),
                        };

                        if mark_idx == Some(last_idx) {
                            writer.push_str("*");
                        }

                        writer.push('`');
                    }
                    _ => {
                        let sep = if last.y == 0 { SEP_NAME } else { SEP_VALUE };
                        writer.push_str(sep);

                        let mut value = Cow::Borrowed(last);

                        if matches!(&last.inner, Value::Mapper(mapper) if mapper.with_status)
                            && data.map.status() == RankStatus::Ranked
                            && data.map.ranked_date().is_some()
                            && self
                                .values
                                .iter()
                                .any(|value| value.kind() == ValueKind::MapRankedDate)
                        {
                            value = Cow::Owned(SettingValue {
                                inner: Value::Mapper(MapperValue { with_status: false }),
                                y: last.y,
                            });
                        }

                        if mark_idx == Some(last_idx) {
                            writer.push_str(mark);
                        }

                        Self::write_field_inner(&value, data, &map_attrs, score_data, writer);

                        if mark_idx == Some(last_idx) {
                            writer.push_str(mark);
                        }
                    }
                }
            }
        }

        let footer_text = (!footer_text.is_empty()).then_some(footer_text);

        (field_name, field_value, footer_text)
    }

    fn write_field_inner(
        value: &SettingValue,
        data: &ScoreEmbedData,
        map_attrs: &BeatmapAttributes,
        score_data: ScoreData,
        writer: &mut String,
    ) {
        match &value.inner {
            Value::Grade => {
                let _ = if value.y == 0 {
                    write!(
                        writer,
                        "{}",
                        GradeFormatter::new(data.score.grade, None, false),
                    )
                } else {
                    write!(
                        writer,
                        "{}",
                        GradeFormatter::new(data.score.grade, data.score.legacy_id, true),
                    )
                };

                // The completion is very hard to calculate for `Catch` because
                // `n_objects` is not correct due to juicestreams so we won't
                // show it for that mode.
                let is_fail = data.score.grade == Grade::F && data.score.mode != GameMode::Catch;

                if is_fail {
                    let n_objects = data.map.n_objects();

                    let completion = if n_objects != 0 {
                        100 * data.score.total_hits() / n_objects
                    } else {
                        100
                    };

                    let _ = write!(writer, "@{completion}%");
                }
            }
            Value::Mods => {
                let _ = write!(writer, "+{}", data.score.mods);
            }
            Value::Score => {
                let _ = write!(writer, "{}", ScoreFormatter::new(&data.score, score_data));
            }
            Value::Accuracy => {
                let _ = write!(writer, "{}%", round(data.score.accuracy));
            }
            Value::ScoreDate => {
                let score_date = data.score.ended_at;

                if value.y == FOOTER_Y {
                    if OffsetDateTime::now_utc() < score_date + DAY {
                        let _ = write!(writer, "{}", HowLongAgoText::new(&score_date));
                    } else {
                        writer.push_str(&score_date.format(&SHORT_NAIVE_DATETIME_FORMAT).unwrap());
                    }
                } else {
                    let _ = write!(writer, "{}", HowLongAgoDynamic::new(&score_date));
                }
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
            Value::MapRankedDate => {
                if let Some(ranked_date) = data.map.ranked_date() {
                    writer.push_str("Ranked ");

                    if OffsetDateTime::now_utc() < ranked_date + DAY {
                        let _ = if value.y == FOOTER_Y {
                            write!(writer, "{}", HowLongAgoText::new(&ranked_date))
                        } else {
                            write!(writer, "{}", HowLongAgoDynamic::new(&ranked_date))
                        };
                    } else if value.y == FOOTER_Y {
                        writer.push_str(&ranked_date.format(&SHORT_NAIVE_DATETIME_FORMAT).unwrap());
                    } else {
                        let _ = write!(writer, "<t:{}:f>", ranked_date.unix_timestamp());
                    }
                }
            }
            Value::Mapper(mapper) => {
                let creator = data.map.creator();

                let _ = if mapper.with_status {
                    write!(writer, "{:?} mapset by {creator}", data.map.status())
                } else {
                    write!(writer, "Mapset by {creator}")
                };
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SettingValue {
    pub inner: Value,
    pub y: u8,
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
            Value::MapRankedDate => ValueKind::MapRankedDate,
            Value::Mapper(_) => ValueKind::Mapper,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Value {
    Grade,
    Mods,
    Score,
    Accuracy,
    ScoreDate,
    Pp(PpValue),
    Combo(ComboValue),
    Hitresults(HitresultsValue),
    Length,
    Bpm(BpmValue),
    Ar,
    Cs,
    Hp,
    Od,
    CountObjects,
    CountSpinners,
    MapRankedDate,
    Mapper(MapperValue),
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
            ValueKind::MapRankedDate => Self::MapRankedDate,
            ValueKind::Mapper => Self::Mapper(Default::default()),
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
                SettingValue {
                    inner: Value::Mapper(Default::default()),
                    y: FOOTER_Y,
                },
                SettingValue {
                    inner: Value::MapRankedDate,
                    y: FOOTER_Y,
                },
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
    Hide,
}

#[derive(Debug)]
pub struct SettingsButtons {
    pub pagination: bool,
    pub render: bool,
    pub miss_analyzer: bool,
}

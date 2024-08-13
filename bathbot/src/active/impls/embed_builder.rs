use std::{
    cmp::{self, Ordering},
    future::ready,
};

use bathbot_model::{
    embed_builder::{
        ComboValue, EmoteTextValue, HitresultsValue, MapperValue, PpValue, ScoreEmbedSettings,
        SettingValue, SettingsButtons, SettingsImage, Value,
    },
    rosu_v2::user::User,
};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::MessageBuilder;
use eyre::{Result, WrapErr};
use futures::future::BoxFuture;
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
    commands::utility::ScoreEmbedDataWrap,
    core::Context,
    manager::redis::RedisData,
    util::{interaction::InteractionComponent, Authored, Emote},
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
                    "buttons" => EmbedSection::Buttons,
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
                    "artist" => ValueKind::Artist,
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
                let last_idx = self
                    .inner
                    .settings
                    .values
                    .iter()
                    .rposition(|value| value.y < SettingValue::FOOTER_Y)
                    .expect("at least one field");

                let last_y = self.inner.settings.values[last_idx].y;

                let value = SettingValue {
                    inner: self.value_kind.into(),
                    y: last_y + 1,
                };

                self.inner.settings.values.insert(last_idx + 1, value);
            }
            "embed_builder_hide_button" => {
                let Some(idx) = self
                    .inner
                    .settings
                    .values
                    .iter()
                    .position(|value| ValueKind::from_setting(value) == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot remove value that's not present"));
                };

                if disable_hide(&self.inner.settings, idx) {
                    return ComponentResult::Err(eyre!("Conditions were not met to hide value"));
                }

                let curr_y = self.inner.settings.values[idx].y;

                let curr_x = self.inner.settings.values[..idx]
                    .iter()
                    .rev()
                    .take_while(|value| value.y == curr_y)
                    .count();

                let next_y = self.inner.settings.values.get(idx + 1).map(|value| value.y);

                if curr_x == 0 && next_y.is_some_and(|next_y| next_y != curr_y) {
                    for value in self.inner.settings.values[idx + 1..].iter_mut() {
                        if value.y == SettingValue::FOOTER_Y {
                            break;
                        }

                        value.y -= 1;
                    }
                }

                self.inner.settings.values.remove(idx);
            }
            "embed_builder_reset_button" => {
                let default = ScoreEmbedSettings::default();
                self.inner.settings.values = default.values;
                self.inner.settings.show_artist = default.show_artist;
            }
            "embed_builder_show_artist_button" => self.inner.settings.show_artist = true,
            "embed_builder_hide_artist_button" => self.inner.settings.show_artist = false,
            "embed_builder_value_left" => {
                let Some(idx) = self
                    .inner
                    .settings
                    .values
                    .iter()
                    .position(|value| ValueKind::from_setting(value) == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                self.inner.settings.values.swap(idx - 1, idx);
            }
            "embed_builder_value_up" => {
                let Some(idx) = self
                    .inner
                    .settings
                    .values
                    .iter()
                    .position(|value| ValueKind::from_setting(value) == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                let can_move = match self.inner.settings.values.get(idx) {
                    Some(value) => value.y > 0,
                    None => false,
                };

                if !can_move {
                    return ComponentResult::Err(eyre!("Conditions were not met to move value up"));
                }

                let curr_y = self.inner.settings.values[idx].y;

                let mut curr_x = 0;
                let mut prev_y = None;
                let mut prev_row_len = 0;

                for prev in self.inner.settings.values[..idx].iter().rev() {
                    if prev.y == curr_y {
                        curr_x += 1;
                    } else if curr_y == SettingValue::FOOTER_Y {
                        prev_y = Some(prev.y);

                        break;
                    } else if let Some(prev_y) = prev_y {
                        if prev_y == prev.y {
                            prev_row_len += 1;
                        } else {
                            break;
                        }
                    } else {
                        prev_y = Some(prev.y);
                        prev_row_len += 1;
                    }
                }

                let to_right_count = self.inner.settings.values[idx + 1..]
                    .iter()
                    .take_while(|value| value.y == curr_y)
                    .count();

                if self.inner.settings.values[idx].y == SettingValue::FOOTER_Y {
                    self.inner.settings.values[idx].y = prev_y.unwrap_or(0) + 1;
                } else {
                    self.inner.settings.values[idx].y -= 1;
                }

                if curr_x == 0 && to_right_count == 0 {
                    for value in self.inner.settings.values[idx + 1..].iter_mut() {
                        if value.y == SettingValue::FOOTER_Y {
                            break;
                        }

                        value.y -= 1;
                    }
                }

                let shift = match prev_row_len.cmp(&curr_x) {
                    Ordering::Less | Ordering::Equal => curr_x,
                    Ordering::Greater => prev_row_len,
                };

                self.inner.settings.values[idx - shift..=idx].rotate_right(1);
            }
            "embed_builder_value_down" => {
                let Some(idx) = self
                    .inner
                    .settings
                    .values
                    .iter()
                    .position(|value| ValueKind::from_setting(value) == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                let curr_y = self.inner.settings.values[idx].y;

                let curr_x = self.inner.settings.values[..idx]
                    .iter()
                    .rev()
                    .take_while(|value| value.y == curr_y)
                    .count();

                let mut to_right_count = 0;
                let mut next_row_len = 0;

                for next in self.inner.settings.values[idx + 1..].iter() {
                    if next.y == curr_y {
                        to_right_count += 1;
                    } else if next.y == curr_y + 1 {
                        next_row_len += 1;
                    } else {
                        break;
                    }
                }

                if curr_x == 0 && to_right_count == 0 {
                    for value in self.inner.settings.values[idx + 1..].iter_mut() {
                        if value.y == SettingValue::FOOTER_Y {
                            break;
                        }

                        value.y -= 1;
                    }

                    if next_row_len == 0 {
                        self.inner.settings.values[idx].y = SettingValue::FOOTER_Y;
                    }
                } else {
                    self.inner.settings.values[idx].y += 1;
                }

                let shift_next_line = if next_row_len > 0 {
                    next_row_len
                } else if curr_x > 0 || to_right_count > 0 {
                    0
                } else {
                    self.inner
                        .settings
                        .values
                        .iter()
                        .rev()
                        .take_while(|value| value.y == SettingValue::FOOTER_Y)
                        .count()
                };

                let shift = 1 + to_right_count + cmp::min(shift_next_line, curr_x);
                self.inner.settings.values[idx..idx + shift].rotate_left(1);
            }
            "embed_builder_value_right" => {
                let Some(idx) = self
                    .inner
                    .settings
                    .values
                    .iter()
                    .position(|value| ValueKind::from_setting(value) == self.value_kind)
                else {
                    return ComponentResult::Err(eyre!("Cannot move value that's not present"));
                };

                let curr_y = self.inner.settings.values[idx].y;
                let next = self.inner.settings.values.get(idx + 1);

                if next.is_some_and(|next| next.y == curr_y) {
                    self.inner.settings.values.swap(idx, idx + 1);
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
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::Pp)
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
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::Combo)
                {
                    value.inner = Value::Combo(ComboValue { max });
                }
            }
            "embed_builder_hitresults_full" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::Hitresults)
                {
                    value.inner = Value::Hitresults(HitresultsValue::Full);
                }
            }
            "embed_builder_hitresults_misses" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::Hitresults)
                {
                    value.inner = Value::Hitresults(HitresultsValue::OnlyMisses);
                }
            }
            "embed_builder_bpm_emote" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::Bpm)
                {
                    value.inner = Value::Bpm(EmoteTextValue::Emote);
                }
            }
            "embed_builder_bpm_text" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::Bpm)
                {
                    value.inner = Value::Bpm(EmoteTextValue::Text);
                }
            }
            "embed_builder_objects_emote" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::CountObjects)
                {
                    value.inner = Value::CountObjects(EmoteTextValue::Emote);
                }
            }
            "embed_builder_objects_text" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::CountObjects)
                {
                    value.inner = Value::CountObjects(EmoteTextValue::Text);
                }
            }
            "embed_builder_spinners_emote" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::CountSpinners)
                {
                    value.inner = Value::CountSpinners(EmoteTextValue::Emote);
                }
            }
            "embed_builder_spinners_text" => {
                if let Some(value) = self
                    .inner
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::CountSpinners)
                {
                    value.inner = Value::CountSpinners(EmoteTextValue::Text);
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
                    .settings
                    .values
                    .iter_mut()
                    .find(|value| ValueKind::from_setting(value) == ValueKind::Mapper)
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

                self.inner.settings.image = match value.as_str() {
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

                self.inner.settings.buttons = SettingsButtons {
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

        let right_order = self
            .inner
            .settings
            .values
            .windows(2)
            .all(|window| window[0].y <= window[1].y);

        if !right_order {
            debug!(values = ?self.inner.settings.values, "Wrong setting values order");
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
            .settings
            .values
            .iter()
            .position(|value| ValueKind::from_setting(value) == self.value_kind);

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
                            kind_option!("Artist", "artist", Artist),
                            kind_option!("Grade", "grade", Grade),
                            kind_option!("Mods", "mods", Mods),
                            kind_option!("Score", "score", Score),
                            kind_option!("Accuracy", "acc", Accuracy),
                            kind_option!("Score date", "score_date", ScoreDate),
                            kind_option!("PP", "pp", Pp),
                            kind_option!("Combo", "combo", Combo),
                            kind_option!("Hitresults", "hitresults", Hitresults),
                            kind_option!("Length", "len", Length),
                            kind_option!("AR", "ar", Ar),
                            kind_option!("CS", "cs", Cs),
                            kind_option!("HP", "hp", Hp),
                            kind_option!("OD", "od", Od),
                            kind_option!("BPM", "bpm", Bpm),
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
                        Some(idx) => disable_hide(&self.inner.settings, idx),
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

                let arrow_row = |idx: Option<usize>| {
                    let (disable_left, disable_up, disable_down, disable_right) =
                        if let Some(idx) = idx {
                            let curr_y = self.inner.settings.values[idx].y;

                            let to_left = self.inner.settings.values[..idx]
                                .iter()
                                .rev()
                                .take_while(|value| value.y == curr_y)
                                .count();

                            let to_right = self.inner.settings.values[idx + 1..]
                                .iter()
                                .take_while(|value| value.y == curr_y)
                                .count();

                            let is_last_row =
                                self.inner.settings.values[idx + to_right + 1..].is_empty();

                            // Disable up if too many values in field name
                            let disable_up = curr_y == 0
                                || (idx == 1
                                    && self
                                        .inner
                                        .settings
                                        .values
                                        .iter()
                                        .take_while(|value| value.y == 0)
                                        .count()
                                        >= 10);

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
                    .settings
                    .values
                    .iter()
                    .position(|value| ValueKind::from_setting(value) == self.value_kind);

                match self.value_kind {
                    ValueKind::None => {}
                    ValueKind::Artist => {
                        components.push(Component::ActionRow(ActionRow {
                            components: vec![
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_show_artist_button".to_owned()),
                                    disabled: self.inner.settings.show_artist,
                                    emoji: None,
                                    label: Some("Show".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_hide_artist_button".to_owned()),
                                    disabled: !self.inner.settings.show_artist,
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
                        }));
                    }
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
                            .and_then(|idx| self.inner.settings.values.get(idx))
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
                            .and_then(|idx| self.inner.settings.values.get(idx))
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
                            .and_then(|idx| self.inner.settings.values.get(idx))
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
                    ValueKind::Bpm => {
                        components.push(show_hide_row(idx));

                        let emote_text = idx
                            .and_then(|idx| self.inner.settings.values.get(idx))
                            .and_then(|value| match value.inner {
                                Value::Bpm(ref emote_text) => Some(emote_text),
                                _ => None,
                            });

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_bpm_emote".to_owned()),
                                    disabled: matches!(
                                        emote_text,
                                        Some(EmoteTextValue::Emote) | None
                                    ),
                                    emoji: Some(Emote::Bpm.reaction_type()),
                                    label: Some("Emote".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_bpm_text".to_owned()),
                                    disabled: matches!(
                                        emote_text,
                                        Some(EmoteTextValue::Text) | None
                                    ),
                                    emoji: None,
                                    label: Some("Text".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                            ],
                        }));

                        components.push(arrow_row(idx));
                    }
                    ValueKind::CountObjects => {
                        components.push(show_hide_row(idx));

                        let emote_text = idx
                            .and_then(|idx| self.inner.settings.values.get(idx))
                            .and_then(|value| match value.inner {
                                Value::CountObjects(ref emote_text) => Some(emote_text),
                                _ => None,
                            });

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_objects_emote".to_owned()),
                                    disabled: matches!(
                                        emote_text,
                                        Some(EmoteTextValue::Emote) | None
                                    ),
                                    emoji: Some(Emote::Bpm.reaction_type()),
                                    label: Some("Emote".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_objects_text".to_owned()),
                                    disabled: matches!(
                                        emote_text,
                                        Some(EmoteTextValue::Text) | None
                                    ),
                                    emoji: None,
                                    label: Some("Text".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                            ],
                        }));

                        components.push(arrow_row(idx));
                    }
                    ValueKind::CountSpinners => {
                        components.push(show_hide_row(idx));

                        let emote_text = idx
                            .and_then(|idx| self.inner.settings.values.get(idx))
                            .and_then(|value| match value.inner {
                                Value::CountSpinners(ref emote_text) => Some(emote_text),
                                _ => None,
                            });

                        components.push(Component::ActionRow(ActionRow {
                            components: vec![
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_spinners_emote".to_owned()),
                                    disabled: matches!(
                                        emote_text,
                                        Some(EmoteTextValue::Emote) | None
                                    ),
                                    emoji: Some(Emote::Bpm.reaction_type()),
                                    label: Some("Emote".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                                Component::Button(Button {
                                    custom_id: Some("embed_builder_spinners_text".to_owned()),
                                    disabled: matches!(
                                        emote_text,
                                        Some(EmoteTextValue::Text) | None
                                    ),
                                    emoji: None,
                                    label: Some("Text".to_owned()),
                                    style: ButtonStyle::Primary,
                                    url: None,
                                }),
                            ],
                        }));

                        components.push(arrow_row(idx));
                    }
                    ValueKind::MapRankedDate => {
                        components.push(show_hide_row(idx));
                        components.push(arrow_row(idx));
                    }
                    ValueKind::Mapper => {
                        components.push(show_hide_row(idx));

                        let mapper = match idx
                            .and_then(|idx| self.inner.settings.values.get(idx))
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
                        default: self.inner.settings.image == SettingsImage::Thumbnail,
                        description: None,
                        emoji: None,
                        label: "Thumbnail".to_owned(),
                        value: "thumbnail".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.settings.image == SettingsImage::Image,
                        description: None,
                        emoji: None,
                        label: "Image".to_owned(),
                        value: "image".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.settings.image == SettingsImage::Hide,
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
                        default: self.inner.settings.buttons.pagination,
                        description: None,
                        emoji: None,
                        label: "Pagination".to_owned(),
                        value: "pagination".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.settings.buttons.render,
                        description: None,
                        emoji: None,
                        label: "Render".to_owned(),
                        value: "render".to_owned(),
                    },
                    SelectMenuOption {
                        default: self.inner.settings.buttons.miss_analyzer,
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
pub enum ValueKind {
    None,
    Artist,
    Grade,
    Mods,
    Score,
    Accuracy,
    ScoreDate,
    Pp,
    Combo,
    Hitresults,
    Length,
    Ar,
    Cs,
    Hp,
    Od,
    Bpm,
    CountObjects,
    CountSpinners,
    MapRankedDate,
    Mapper,
}

impl ValueKind {
    pub fn from_setting(value: &SettingValue) -> Self {
        match value.inner {
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
            Value::CountObjects(_) => ValueKind::CountObjects,
            Value::CountSpinners(_) => ValueKind::CountSpinners,
            Value::MapRankedDate => ValueKind::MapRankedDate,
            Value::Mapper(_) => ValueKind::Mapper,
        }
    }
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
            ValueKind::CountObjects => Self::CountObjects(Default::default()),
            ValueKind::CountSpinners => Self::CountSpinners(Default::default()),
            ValueKind::MapRankedDate => Self::MapRankedDate,
            ValueKind::Mapper => Self::Mapper(Default::default()),
            ValueKind::Artist | ValueKind::None => unreachable!(),
        }
    }
}

fn disable_hide(settings: &ScoreEmbedSettings, idx: usize) -> bool {
    match settings.values.get(idx) {
        Some(value) => match value.y {
            // disable hide button if first row has only one value
            0 => settings.values.get(1).map_or(true, |value| value.y != 0),
            _ => false,
        },
        None => true,
    }
}

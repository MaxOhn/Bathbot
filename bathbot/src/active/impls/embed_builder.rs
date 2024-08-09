use std::future::ready;

use bathbot_model::{
    command_fields::{
        ScoreEmbedButtons, ScoreEmbedFooter, ScoreEmbedHitResults, ScoreEmbedImage,
        ScoreEmbedMapInfo, ScoreEmbedPp, ScoreEmbedSettings,
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
    util::{interaction::InteractionComponent, Authored},
};

pub struct ScoreEmbedBuilderActive {
    inner: SingleScorePagination,
    content: ContentStatus,
    section: EmbedSection,
    option_kind: ScoreDataOptionKind,
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
            option_kind: ScoreDataOptionKind::None,
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
                        "Missing value for score embed builder menu `{}`",
                        component.data.custom_id
                    ));
                };

                self.section = match value.as_str() {
                    "image" => EmbedSection::Image,
                    "buttons" => EmbedSection::Buttons,
                    _ => {
                        return ComponentResult::Err(eyre!(
                            "Invalid value `{value}` for score embed builder menu `{}`",
                            component.data.custom_id
                        ))
                    }
                };
            }
            "embed_builder_image_button" => self.inner.settings.image = ScoreEmbedImage::Image,
            "embed_builder_thumbnail_button" => {
                self.inner.settings.image = ScoreEmbedImage::Thumbnail
            }
            "embed_builder_no_image_button" => self.inner.settings.image = ScoreEmbedImage::None,
            "embed_builder_max_pp_button" => self.inner.settings.pp = ScoreEmbedPp::Max,
            "embed_builder_if_fc_button" => self.inner.settings.pp = ScoreEmbedPp::IfFc,
            "embed_builder_map_info" => {
                let mut len = false;
                let mut ar = false;
                let mut cs = false;
                let mut od = false;
                let mut hp = false;
                let mut bpm = false;
                let mut n_obj = false;
                let mut n_spin = false;

                for value in component.data.values.iter() {
                    match value.as_str() {
                        "len" => len = true,
                        "ar" => ar = true,
                        "cs" => cs = true,
                        "od" => od = true,
                        "hp" => hp = true,
                        "bpm" => bpm = true,
                        "n_obj" => n_obj = true,
                        "n_spin" => n_spin = true,
                        _ => {
                            return ComponentResult::Err(eyre!(
                                "Invalid value `{value}` for score embed builder menu `{}`",
                                component.data.custom_id
                            ))
                        }
                    }
                }

                self.inner.settings.map_info = ScoreEmbedMapInfo {
                    len,
                    ar,
                    cs,
                    od,
                    hp,
                    bpm,
                    n_obj,
                    n_spin,
                };
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
                                "Invalid value `{value}` for score embed builder menu `{}`",
                                component.data.custom_id
                            ))
                        }
                    }
                }

                self.inner.settings.buttons = ScoreEmbedButtons {
                    pagination,
                    render,
                    miss_analyzer,
                }
            }
            "embed_builder_hitresults_button" => {
                self.inner.settings.hitresults = ScoreEmbedHitResults::Full
            }
            "embed_builder_misses_button" => {
                self.inner.settings.hitresults = ScoreEmbedHitResults::OnlyMisses
            }
            "embed_builder_score_date_button" => {
                self.inner.settings.footer = ScoreEmbedFooter::WithScoreDate
            }
            "embed_builder_ranked_date_button" => {
                self.inner.settings.footer = ScoreEmbedFooter::WithMapRankedDate
            }
            "embed_builder_no_footer_button" => self.inner.settings.footer = ScoreEmbedFooter::Hide,
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

        Box::pin(self.inner.async_build_page(content))
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

        macro_rules! option_value {
            ( $label:literal, $value:literal, $variant:ident ) => {
                SelectMenuOption {
                    default: matches!(self.option_kind, ScoreDataOptionKind::$variant),
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
                let value_options = vec![
                    option_value!("Grade", "grade", Grade),
                    option_value!("Mods", "mods", Mods),
                    option_value!("Score", "score", Score),
                    option_value!("Accuracy", "acc", Accuracy),
                    option_value!("Score date", "score_date", ScoreDate),
                    option_value!("PP", "pp", Pp),
                    option_value!("Combo", "combo", Combo),
                    option_value!("Hitresults", "hitresults", Hitresults),
                    option_value!("Ratio", "ratio", Ratio),
                    option_value!("Length", "length", Length),
                    option_value!("BPM", "bpm", Bpm),
                    option_value!("AR", "ar", Ar),
                    option_value!("CS", "cs", Cs),
                    option_value!("HP", "hp", Hp),
                    option_value!("OD", "od", Od),
                    option_value!("Count objects", "objects", CountObjects),
                    option_value!("Count sliders", "sliders", CountSliders),
                    option_value!("Count spinners", "spinners", CountSpinners),
                    option_value!("Map ranked date", "ranked_date", MapRankedDate),
                    option_value!("Mapper", "mapper", Mapper),
                ];

                components.push(Component::ActionRow(ActionRow {
                    components: vec![Component::SelectMenu(SelectMenu {
                        custom_id: "embed_builder_value".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options: value_options,
                        placeholder: None,
                    })],
                }));

                let inlined_row = |anchor: Anchor| {
                    Component::ActionRow(ActionRow {
                        components: vec![
                            Component::Button(Button {
                                custom_id: Some("embed_builder_inlined_text".to_owned()),
                                disabled: matches!(anchor, Anchor::InlinedText),
                                emoji: None,
                                label: Some("Inlined text".to_owned()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_inlined_field".to_owned()),
                                disabled: matches!(anchor, Anchor::InlinedField),
                                emoji: None,
                                label: Some("Inlined field".to_owned()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_single_field".to_owned()),
                                disabled: matches!(anchor, Anchor::SingleField),
                                emoji: None,
                                label: Some("Single field".to_owned()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_hide".to_owned()),
                                disabled: matches!(anchor, Anchor::Hide),
                                emoji: None,
                                label: Some("Hide".to_owned()),
                                style: ButtonStyle::Primary,
                                url: None,
                            }),
                        ],
                    })
                };

                let arrow_row = |dims: Option<(i32, i32, Dim)>| {
                    Component::ActionRow(ActionRow {
                        components: vec![
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_left".to_owned()),
                                disabled: dims.as_ref().is_some_and(|(x, _, dim)| *x <= dim.min_x),
                                emoji: Some(ReactionType::Unicode {
                                    name: "◀".to_owned(),
                                }),
                                label: Some("Left".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_up".to_owned()),
                                disabled: dims.as_ref().is_some_and(|(_, y, dim)| *y <= dim.min_y),
                                emoji: Some(ReactionType::Unicode {
                                    name: "�".to_owned(),
                                }),
                                label: Some("Up".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_down".to_owned()),
                                disabled: dims.as_ref().is_some_and(|(_, y, dim)| *y >= dim.max_y),
                                emoji: Some(ReactionType::Unicode {
                                    name: "�".to_owned(),
                                }),
                                label: Some("Down".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                            Component::Button(Button {
                                custom_id: Some("embed_builder_value_right".to_owned()),
                                disabled: dims.as_ref().is_some_and(|(x, _, dim)| *x >= dim.max_x),
                                emoji: Some(ReactionType::Unicode {
                                    name: "▶".to_owned(),
                                }),
                                label: Some("Right".to_owned()),
                                style: ButtonStyle::Success,
                                url: None,
                            }),
                        ],
                    })
                };

                macro_rules! find_value {
                    ( $list:ident, $pat:pat ) => {
                        self.inner
                            .new_settings
                            .$list
                            .iter()
                            .find(|value| matches!(value.kind, $pat))
                    };
                }

                macro_rules! push_basic_buttons {
                    ( $variant:ident ) => {{
                        let (anchor, dims) = if let Some(value) =
                            find_value!(combined_field, ScoreDataOption::$variant)
                        {
                            let dim = self.inner.new_settings.combined_dim();

                            (Anchor::InlinedText, Some((value.x, value.y, dim)))
                        } else if let Some(value) =
                            find_value!(separate_fields, ScoreDataOption::$variant)
                        {
                            let dim = self.inner.new_settings.separate_dim();

                            (Anchor::InlinedField, Some((value.x, value.y, dim)))
                        } else {
                            (Anchor::Hide, None)
                        };

                        components.push(inlined_row(anchor));
                        components.push(arrow_row(dims));
                    }};
                }

                match self.option_kind {
                    ScoreDataOptionKind::None => {}
                    ScoreDataOptionKind::Grade => push_basic_buttons!(Grade),
                    ScoreDataOptionKind::Mods => push_basic_buttons!(Mods),
                    ScoreDataOptionKind::Score => push_basic_buttons!(Score),
                    ScoreDataOptionKind::Accuracy => push_basic_buttons!(Accuracy),
                    ScoreDataOptionKind::ScoreDate => push_basic_buttons!(ScoreDate),
                    ScoreDataOptionKind::Ratio => push_basic_buttons!(Ratio),
                    ScoreDataOptionKind::Length => push_basic_buttons!(Length),
                    ScoreDataOptionKind::Ar => push_basic_buttons!(Ar),
                    ScoreDataOptionKind::Cs => push_basic_buttons!(Cs),
                    ScoreDataOptionKind::Hp => push_basic_buttons!(Hp),
                    ScoreDataOptionKind::Od => push_basic_buttons!(Od),
                    ScoreDataOptionKind::CountObjects => push_basic_buttons!(CountObjects),
                    ScoreDataOptionKind::CountSliders => push_basic_buttons!(CountSliders),
                    ScoreDataOptionKind::CountSpinners => push_basic_buttons!(CountSpinners),
                    ScoreDataOptionKind::MapRankedDate => push_basic_buttons!(MapRankedDate),
                    ScoreDataOptionKind::Mapper => push_basic_buttons!(Mapper),
                    ScoreDataOptionKind::Pp => {
                        let (anchor, dims) = if let Some(value) =
                            find_value!(combined_field, ScoreDataOption::Pp(_))
                        {
                            let dim = self.inner.new_settings.combined_dim();

                            (Anchor::InlinedText, Some((value.x, value.y, dim)))
                        } else if let Some(value) =
                            find_value!(separate_fields, ScoreDataOption::Pp(_))
                        {
                            let dim = self.inner.new_settings.separate_dim();

                            (Anchor::InlinedField, Some((value.x, value.y, dim)))
                        } else {
                            (Anchor::Hide, None)
                        };

                        components.push(inlined_row(anchor));

                        // TODO

                        components.push(arrow_row(dims));
                    }
                    ScoreDataOptionKind::Combo => {
                        let (anchor, dims) = if let Some(value) =
                            find_value!(combined_field, ScoreDataOption::Combo(_))
                        {
                            let dim = self.inner.new_settings.combined_dim();

                            (Anchor::InlinedText, Some((value.x, value.y, dim)))
                        } else if let Some(value) =
                            find_value!(separate_fields, ScoreDataOption::Combo(_))
                        {
                            let dim = self.inner.new_settings.separate_dim();

                            (Anchor::InlinedField, Some((value.x, value.y, dim)))
                        } else {
                            (Anchor::Hide, None)
                        };

                        components.push(inlined_row(anchor));

                        // TODO

                        components.push(arrow_row(dims));
                    }
                    ScoreDataOptionKind::Hitresults => {
                        let (anchor, dims) = if let Some(value) =
                            find_value!(combined_field, ScoreDataOption::Hitresults(_))
                        {
                            let dim = self.inner.new_settings.combined_dim();

                            (Anchor::InlinedText, Some((value.x, value.y, dim)))
                        } else if let Some(value) =
                            find_value!(separate_fields, ScoreDataOption::Hitresults(_))
                        {
                            let dim = self.inner.new_settings.separate_dim();

                            (Anchor::InlinedField, Some((value.x, value.y, dim)))
                        } else {
                            (Anchor::Hide, None)
                        };

                        components.push(inlined_row(anchor));

                        // TODO

                        components.push(arrow_row(dims));
                    }
                    ScoreDataOptionKind::Bpm => {
                        let (anchor, dims) = if let Some(value) =
                            find_value!(combined_field, ScoreDataOption::Bpm(_))
                        {
                            let dim = self.inner.new_settings.combined_dim();

                            (Anchor::InlinedText, Some((value.x, value.y, dim)))
                        } else if let Some(value) =
                            find_value!(separate_fields, ScoreDataOption::Bpm(_))
                        {
                            let dim = self.inner.new_settings.separate_dim();

                            (Anchor::InlinedField, Some((value.x, value.y, dim)))
                        } else {
                            (Anchor::Hide, None)
                        };

                        components.push(inlined_row(anchor));

                        // TODO

                        components.push(arrow_row(dims));
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
enum ScoreDataOptionKind {
    None,
    Grade,
    Mods,
    Score,
    Accuracy,
    ScoreDate,
    Pp,
    Combo,
    Hitresults,
    Ratio,
    Length,
    Bpm,
    Ar,
    Cs,
    Hp,
    Od,
    CountObjects,
    CountSliders,
    CountSpinners,
    MapRankedDate,
    Mapper,
}

pub enum ScoreDataOption {
    Grade,
    Mods,
    Score,
    Accuracy,
    ScoreDate,
    Pp(ScoreDataOptionPp),
    Combo(ScoreDataOptionCombo),
    Hitresults(ScoreDataOptionHitresults),
    Ratio,
    Length,
    Bpm(ScoreDataOptionBpm),
    Ar,
    Cs,
    Hp,
    Od,
    CountObjects,
    CountSliders,
    CountSpinners,
    MapRankedDate,
    Mapper,
}

pub struct ScoreDataOptionPp {
    pub max: bool,
    pub if_fc: bool,
}

pub struct ScoreDataOptionCombo {
    pub max: bool,
}

pub enum ScoreDataOptionHitresults {
    Full,
    OnlyMisses,
}

pub enum ScoreDataOptionBpm {
    Emote,
    Text,
}

impl ScoreDataOption {
    pub fn field_name(&self) -> &'static str {
        todo!()
    }
}

pub struct SettingValue {
    pub kind: ScoreDataOption,
    pub x: i32,
    pub y: i32,
}

pub struct SettingFieldValue {
    pub kind: ScoreDataOption,
    pub inline: bool,
    pub x: i32,
    pub y: i32,
}

pub struct Settings {
    pub combined_field: Vec<SettingValue>,
    pub separate_fields: Vec<SettingValue>,
    pub image: SettingsImage,
    pub buttons: SettingsButtons,
}

impl Settings {
    fn combined_dim(&self) -> Dim {
        let mut dim = Dim {
            min_x: i32::MAX,
            max_x: i32::MIN,
            min_y: i32::MAX,
            max_y: i32::MIN,
        };

        for value in self.combined_field.iter() {
            dim.min_x = dim.min_x.min(value.x);
            dim.max_x = dim.max_x.max(value.x);
            dim.min_y = dim.min_y.min(value.y);
            dim.max_y = dim.max_y.max(value.y);
        }

        dim
    }

    fn separate_dim(&self) -> Dim {
        let mut dim = Dim {
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
        };

        for value in self.separate_fields.iter() {
            dim.min_x = dim.min_x.min(value.x);
            dim.max_x = dim.max_x.max(value.x);
            dim.min_y = dim.min_y.min(value.y);
            dim.max_y = dim.max_y.max(value.y);
        }

        dim
    }
}

struct Dim {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            combined_field: vec![
                SettingValue {
                    kind: ScoreDataOption::Grade,
                    x: 0,
                    y: 0,
                },
                SettingValue {
                    kind: ScoreDataOption::Mods,
                    x: 1,
                    y: 0,
                },
                SettingValue {
                    kind: ScoreDataOption::Score,
                    x: 2,
                    y: 0,
                },
                SettingValue {
                    kind: ScoreDataOption::Accuracy,
                    x: 3,
                    y: 0,
                },
                SettingValue {
                    kind: ScoreDataOption::ScoreDate,
                    x: 4,
                    y: 0,
                },
                SettingValue {
                    kind: ScoreDataOption::Pp(ScoreDataOptionPp {
                        max: true,
                        if_fc: true,
                    }),
                    x: 0,
                    y: 1,
                },
                SettingValue {
                    kind: ScoreDataOption::Combo(ScoreDataOptionCombo { max: true }),
                    x: 1,
                    y: 1,
                },
                SettingValue {
                    kind: ScoreDataOption::Hitresults(ScoreDataOptionHitresults::OnlyMisses),
                    x: 2,
                    y: 1,
                },
                SettingValue {
                    kind: ScoreDataOption::Length,
                    x: 0,
                    y: 2,
                },
                SettingValue {
                    kind: ScoreDataOption::Cs,
                    x: 1,
                    y: 2,
                },
                SettingValue {
                    kind: ScoreDataOption::Ar,
                    x: 2,
                    y: 2,
                },
                SettingValue {
                    kind: ScoreDataOption::Od,
                    x: 3,
                    y: 2,
                },
                SettingValue {
                    kind: ScoreDataOption::Hp,
                    x: 4,
                    y: 2,
                },
                SettingValue {
                    kind: ScoreDataOption::Bpm(ScoreDataOptionBpm::Emote),
                    x: 5,
                    y: 2,
                },
                SettingValue {
                    kind: ScoreDataOption::Mapper,
                    x: 0,
                    y: 3,
                },
                SettingValue {
                    kind: ScoreDataOption::MapRankedDate,
                    x: 1,
                    y: 3,
                },
            ],
            separate_fields: Vec::new(),
            image: SettingsImage::Thumbnail,
            buttons: SettingsButtons {
                pagination: true,
                render: true,
                miss_analyzer: true,
            },
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SettingsImage {
    Thumbnail,
    Image,
    ImageWithStrains,
    Hide,
}

enum Anchor {
    InlinedText,
    InlinedField,
    SingleField,
    Hide,
}

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

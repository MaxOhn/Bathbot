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
        Component,
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
            "score_embed_builder_image_button" => {
                self.inner.settings.image = ScoreEmbedImage::Image
            }
            "score_embed_builder_thumbnail_button" => {
                self.inner.settings.image = ScoreEmbedImage::Thumbnail
            }
            "score_embed_builder_no_image_button" => {
                self.inner.settings.image = ScoreEmbedImage::None
            }
            "score_embed_builder_max_pp_button" => self.inner.settings.pp = ScoreEmbedPp::Max,
            "score_embed_builder_if_fc_button" => self.inner.settings.pp = ScoreEmbedPp::IfFc,
            "score_embed_builder_map_info" => {
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
            "score_embed_builder_buttons" => {
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
            "score_embed_builder_hitresults_button" => {
                self.inner.settings.hitresults = ScoreEmbedHitResults::Full
            }
            "score_embed_builder_misses_button" => {
                self.inner.settings.hitresults = ScoreEmbedHitResults::OnlyMisses
            }
            "score_embed_builder_score_date_button" => {
                self.inner.settings.footer = ScoreEmbedFooter::WithScoreDate
            }
            "score_embed_builder_ranked_date_button" => {
                self.inner.settings.footer = ScoreEmbedFooter::WithMapRankedDate
            }
            "score_embed_builder_no_footer_button" => {
                self.inner.settings.footer = ScoreEmbedFooter::Hide
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

        Box::pin(self.inner.async_build_page(content))
    }

    fn build_components(&self) -> Vec<Component> {
        macro_rules! menu_option {
            ( $( $field:ident ).+: $label:literal ) => {
                SelectMenuOption {
                    default: self.inner.settings. $( $field ).*,
                    description: None,
                    emoji: None,
                    label: $label.to_owned(),
                    value: menu_option!( @ $( $field ).*).to_owned(),
                }
            };

            // Stringifying the last identifier of the field sequence
            ( @ $first:ident . $( $rest:ident ).+ ) => {
                menu_option!( @ $( $rest )* )
            };
            ( @ $field:ident ) => {
                stringify!($field)
            };
        }

        let map_info_options = vec![
            menu_option!(map_info.len: "Length"),
            menu_option!(map_info.ar: "AR"),
            menu_option!(map_info.cs: "CS"),
            menu_option!(map_info.od: "OD"),
            menu_option!(map_info.hp: "HP"),
            menu_option!(map_info.bpm: "BPM"),
            menu_option!(map_info.n_obj: "Object count"),
            menu_option!(map_info.n_spin: "Spinner count"),
        ];

        let button_options = vec![
            menu_option!(buttons.pagination: "Pagination"),
            menu_option!(buttons.render: "Render button"),
            menu_option!(buttons.miss_analyzer: "Miss analyzer"),
        ];

        vec![
            Component::ActionRow(ActionRow {
                components: vec![
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_image_button".to_owned()),
                        disabled: self.inner.settings.image == ScoreEmbedImage::Image,
                        emoji: None,
                        label: Some("Image".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_thumbnail_button".to_owned()),
                        disabled: self.inner.settings.image == ScoreEmbedImage::Thumbnail,
                        emoji: None,
                        label: Some("Thumbnail".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_no_image_button".to_owned()),
                        disabled: self.inner.settings.image == ScoreEmbedImage::None,
                        emoji: None,
                        label: Some("No image".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_max_pp_button".to_owned()),
                        disabled: self.inner.settings.footer == ScoreEmbedFooter::WithScoreDate
                            || self.inner.settings.pp == ScoreEmbedPp::Max
                            || self.inner.settings.hitresults == ScoreEmbedHitResults::OnlyMisses,
                        emoji: None,
                        label: Some("Max PP".to_owned()),
                        style: ButtonStyle::Secondary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_if_fc_button".to_owned()),
                        disabled: self.inner.settings.footer == ScoreEmbedFooter::WithScoreDate
                            || self.inner.settings.pp == ScoreEmbedPp::IfFc
                            || self.inner.settings.hitresults == ScoreEmbedHitResults::OnlyMisses,
                        emoji: None,
                        label: Some("If-FC PP".to_owned()),
                        style: ButtonStyle::Secondary,
                        url: None,
                    }),
                ],
            }),
            Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "score_embed_builder_map_info".to_owned(),
                    disabled: false,
                    max_values: Some(map_info_options.len() as u8),
                    min_values: Some(0),
                    options: map_info_options,
                    placeholder: Some("Hide map info".to_owned()),
                })],
            }),
            Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "score_embed_builder_buttons".to_owned(),
                    disabled: false,
                    max_values: Some(button_options.len() as u8),
                    min_values: Some(0),
                    options: button_options,
                    placeholder: Some("Without buttons".to_owned()),
                })],
            }),
            Component::ActionRow(ActionRow {
                components: vec![
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_hitresults_button".to_owned()),
                        disabled: self.inner.settings.hitresults == ScoreEmbedHitResults::Full,
                        emoji: None,
                        label: Some("Hitresults".to_owned()),
                        style: ButtonStyle::Secondary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_misses_button".to_owned()),
                        disabled: self.inner.settings.hitresults
                            == ScoreEmbedHitResults::OnlyMisses,
                        emoji: None,
                        label: Some("Only misses".to_owned()),
                        style: ButtonStyle::Secondary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_score_date_button".to_owned()),
                        disabled: self.inner.settings.footer == ScoreEmbedFooter::WithScoreDate,
                        emoji: None,
                        label: Some("Score date".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_ranked_date_button".to_owned()),
                        disabled: self.inner.settings.footer == ScoreEmbedFooter::WithMapRankedDate,
                        emoji: None,
                        label: Some("Ranked date".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("score_embed_builder_no_footer_button".to_owned()),
                        disabled: self.inner.settings.footer == ScoreEmbedFooter::Hide,
                        emoji: None,
                        label: Some("Hide footer".to_owned()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                ],
            }),
        ]
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

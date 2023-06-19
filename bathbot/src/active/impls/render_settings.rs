#![allow(unused)]

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    future::ready,
    mem,
    sync::Arc,
};

use bathbot_util::{
    fields,
    modal::{ModalBuilder, TextInputBuilder},
    numbers::round,
    CowUtils, EmbedBuilder,
};
use eyre::{ContextCompat, Result, WrapErr};
use futures::future::BoxFuture;
use rosu_render::model::{RenderOptions, RenderSkinOption};
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption},
        Component,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    core::Context,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Authored, ComponentExt, ModalExt,
    },
};

pub struct RenderSettingsActive {
    skin: RenderSkinOption<'static>,
    settings: RenderOptions,
    group: SettingsGroup,
    skin_status: SkinStatus,
    content: Option<&'static str>,
    defer_next: bool,
    msg_owner: Id<UserMarker>,
}

impl RenderSettingsActive {
    pub fn new(
        skin: RenderSkinOption<'static>,
        settings: RenderOptions,
        content: Option<&'static str>,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            skin,
            settings,
            group: SettingsGroup::default(),
            skin_status: SkinStatus::default(),
            content,
            defer_next: false,
            msg_owner,
        }
    }

    async fn handle_group_menu(
        &mut self,
        ctx: Arc<Context>,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let Some(value) = component.data.values.pop() else {
            return ComponentResult::Err(eyre!("Missing value for settings group menu"));
        };

        self.group = match value.as_str() {
            "skin" => SettingsGroup::Skin,
            "audio" => SettingsGroup::Audio,
            "hud" => SettingsGroup::Hud,
            "cursor" => SettingsGroup::Cursor,
            "background" => SettingsGroup::Background,
            "intro" => SettingsGroup::Intro,
            "objects" => SettingsGroup::Objects,
            other => {
                return ComponentResult::Err(eyre!("Unknown settings group menu option `{other}`"))
            }
        };

        ComponentResult::BuildPage
    }

    async fn handle_edit_menu(
        &mut self,
        ctx: Arc<Context>,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let Some(value) = component.data.values.pop() else {
            return ComponentResult::Err(eyre!("Missing value for settings edit menu"));
        };

        fn create_modal(custom_id: &str, label: &str, ty: &str) -> ModalBuilder {
            let input = TextInputBuilder::new(custom_id, label)
                .required(true)
                .placeholder(ty);

            ModalBuilder::new(custom_id, "Render settings").input(input)
        }

        let modal = match value.as_str() {
            "skin" => create_modal("skin", "Specify a skin name", "Name"),
            "use_skin_cursor" => create_modal("use_skin_cursor", "Use the skin cursor", "Boolean"),
            "use_skin_hitsounds" => {
                create_modal("use_skin_hitsounds", "Use the skin hitsounds", "Boolean")
            }
            "global_volume" => create_modal(
                "global_volume",
                "Specify a global volume",
                "Integer between 0 and 100",
            ),
            "music_volume" => create_modal(
                "music_volume",
                "Specify a music volume",
                "Integer between 0 and 100",
            ),
            "hitsound_volume" => create_modal(
                "hitsound_volume",
                "Specify a hitsound volume",
                "Integer between 0 and 100",
            ),
            "play_nightcore_samples" => create_modal(
                "play_nightcore_samples",
                "Play nightcore hitsounds",
                "Boolean",
            ),
            "show_hit_error_meter" => create_modal(
                "show_hit_error_meter",
                "Show the hit error meter",
                "Boolean",
            ),
            "show_aim_error_meter" => create_modal(
                "show_aim_error_meter",
                "Show the aim error meter",
                "Boolean",
            ),
            "show_hp_bar" => create_modal("show_hp_bar", "Show the HP bar", "Boolean"),
            "show_key_overlay" => {
                create_modal("show_key_overlay", "Show the key overlay", "Boolean")
            }
            "show_borders" => create_modal("show_borders", "Show the playfield borders", "Boolean"),
            "show_mods" => create_modal("show_mods", "Show mods during the game", "Boolean"),
            "show_score" => create_modal("show_score", "Show the score", "Boolean"),
            "show_combo_counter" => {
                create_modal("show_combo_counter", "Show the combo counter", "Boolean")
            }
            "show_pp_counter" => create_modal("show_pp_counter", "Show the pp counter", "Boolean"),
            "show_hit_counter" => create_modal(
                "show_hit_counter",
                "Show a hit counter (100, 50, miss)",
                "Boolean",
            ),
            "show_unstable_rate" => {
                create_modal("show_unstable_rate", "Show the unstable rate", "Boolean")
            }
            "show_scoreboard" => create_modal("show_scoreboard", "Show the scoreboard", "Boolean"),
            "show_avatars_on_scoreboard" => create_modal(
                "show_avatars_on_scoreboard",
                "Show user avatars in the scoreboard",
                "Boolean",
            ),
            "cursor_rainbow" => {
                create_modal("cursor_rainbow", "Make the cursor rainbow", "Boolean")
            }
            "cursor_trail_glow" => {
                create_modal("cursor_trail_glow", "Have a glow with the trail", "Boolean")
            }
            "cursor_size" => create_modal(
                "cursor_size",
                "Specify a cursor size",
                "Number between 0.5 and 2.0",
            ),
            "cursor_trail" => create_modal("cursor_trail", "Show the cursor trail", "Boolean"),
            "cursor_ripples" => create_modal(
                "cursor_ripples",
                "Show cursor ripples on keypress",
                "Boolean",
            ),
            "cursor_scale_to_cs" => create_modal(
                "cursor_scale_to_cs",
                "Scale cursor to circle size",
                "Boolean",
            ),
            "intro_bg_dim" => create_modal(
                "intro_bg_dim",
                "Specify a BG dim for the intro",
                "Integer between 0 and 100",
            ),
            "ingame_bg_dim" => create_modal(
                "ingame_bg_dim",
                "Specify a BG dim in the play",
                "Integer between 0 and 100",
            ),
            "break_bg_dim" => create_modal(
                "break_bg_dim",
                "Specify a BG dim during breaks",
                "Integer between 0 and 100",
            ),
            "bg_parallax" => create_modal("bg_parallax", "Add a parallax effect", "Boolean"),
            "load_storyboard" => create_modal("load_storyboard", "Load the storyboard", "Boolean"),
            "load_video" => create_modal("load_video", "Load the video", "Boolean"),
            "skip_intro" => create_modal("skip_intro", "Skip the intro", "Boolean"),
            "show_danser_logo" => create_modal(
                "show_danser_logo",
                "Show danser logo in the intro",
                "Boolean",
            ),
            "seizure_warning" => create_modal(
                "seizure_warning",
                "Show seizure warning in the intro",
                "Boolean",
            ),
            "objects_rainbow" => {
                create_modal("objects_rainbow", "Make the objects rainbow", "Boolean")
            }
            "flash_objects" => create_modal(
                "flash_objects",
                "Make the objects flash to the beat",
                "Boolean",
            ),
            "slider_merge" => create_modal("slider_merge", "Merge sliders", "Boolean"),
            "slider_snaking_in" => {
                create_modal("slider_snaking_in", "Have sliders snake in", "Boolean")
            }
            "slider_snaking_out" => {
                create_modal("slider_snaking_out", "Have sliders snake out", "Boolean")
            }
            "use_slider_hitcircle_color" => create_modal(
                "use_slider_hitcircle_color",
                "Sliders have the same color as hitcircles",
                "Boolean",
            ),
            "draw_combo_numbers" => create_modal(
                "draw_combo_numbers",
                "Show the combo numbers in objets",
                "Boolean",
            ),
            "beat_scaling" => create_modal("beat_scaling", "Scale objects to the beat", "Boolean"),
            "use_beatmap_colors" => create_modal(
                "use_beatmap_colors",
                "Use the beatmap combo colors",
                "Boolean",
            ),
            "draw_follow_points" => create_modal(
                "draw_follow_points",
                "Draw follow points between objects",
                "Boolean",
            ),
            other => {
                return ComponentResult::Err(eyre!("Unknown settings edit menu option `{other}`"))
            }
        };

        ComponentResult::CreateModal(modal)
    }

    async fn async_handle_modal(
        &mut self,
        ctx: &Context,
        modal: &mut InteractionModal,
    ) -> Result<()> {
        let mut input = modal
            .data
            .components
            .pop()
            .and_then(|mut row| row.components.pop())
            .and_then(|component| component.value)
            .wrap_err(eyre!("Missing input in modal"))?;

        let mut deferred = false;

        macro_rules! parse_input {
            (bool: $field:ident) => {
                self.settings.$field = match input.cow_to_ascii_lowercase().as_ref() {
                    "true" | "t" | "1" | "yes" | "y" => true,
                    "false" | "f" | "0" | "no" | "n" => false,
                    _ => bail!(
                        "Invalid render settings input `{input}` for `{field}`",
                        field = stringify!($field)
                    ),
                }
            };
            (percent: $field:ident) => {
                self.settings.$field = input
                    .parse::<u8>()
                    .map_err(|_| {
                        eyre!(
                            "Invalid render settings input `{input}` for `{field}`",
                            field = stringify!($field)
                        )
                    })?
                    .clamp(0, 100)
            };
        }

        match modal.data.custom_id.as_str() {
            "skin" => {
                if let Err(err) = modal.defer(ctx).await {
                    warn!("Failed to defer modal");
                }

                deferred = true;
                let mut chars = input.char_indices().rev();

                // truncate suffixed whitespace
                if let Some((_, ' ')) = chars.next() {
                    if let Some((i, c)) = chars.find(|(_, c)| *c != ' ') {
                        input.truncate(i + c.len_utf8());
                    }
                }

                // We're not simply propagating errors because the modal must be deferred
                // already so we need to respond properly
                match ctx.ordr().client().skin_list().search(&input).await {
                    Ok(mut skin_list) => match skin_list.skins.pop() {
                        Some(skin) => {
                            self.skin.skin_name = skin.presentation_name.into_string().into();
                        }
                        None => self.skin_status = SkinStatus::NotFound,
                    },
                    Err(err) => {
                        warn!(?err, "Failed to search for skin `{input}`");
                        self.skin_status = SkinStatus::Err;
                    }
                }
            }
            "use_skin_cursor" => parse_input!(bool: use_skin_cursor),
            "use_skin_hitsounds" => parse_input!(bool: use_skin_hitsounds),
            "global_volume" => parse_input!(percent: global_volume),
            "music_volume" => parse_input!(percent: music_volume),
            "hitsound_volume" => parse_input!(percent: hitsound_volume),
            "play_nightcore_samples" => parse_input!(bool: play_nightcore_samples),
            "show_hit_error_meter" => parse_input!(bool: show_hit_error_meter),
            "show_aim_error_meter" => parse_input!(bool: show_aim_error_meter),
            "show_hp_bar" => parse_input!(bool: show_hp_bar),
            "show_key_overlay" => parse_input!(bool: show_key_overlay),
            "show_borders" => parse_input!(bool: show_borders),
            "show_mods" => parse_input!(bool: show_mods),
            "show_score" => parse_input!(bool: show_score),
            "show_combo_counter" => parse_input!(bool: show_combo_counter),
            "show_pp_counter" => parse_input!(bool: show_pp_counter),
            "show_hit_counter" => parse_input!(bool: show_hit_counter),
            "show_unstable_rate" => parse_input!(bool: show_unstable_rate),
            "show_scoreboard" => parse_input!(bool: show_scoreboard),
            "show_avatars_on_scoreboard" => parse_input!(bool: show_avatars_on_scoreboard),
            "cursor_rainbow" => parse_input!(bool: cursor_rainbow),
            "cursor_trail_glow" => parse_input!(bool: cursor_trail_glow),
            "cursor_size" => {
                self.settings.cursor_size = input
                    .parse::<f32>()
                    .map_err(|_| {
                        eyre!("Invalid render settings input `{input}` for `cursor_size`")
                    })?
                    .clamp(0.5, 2.0)
            }
            "cursor_trail" => parse_input!(bool: cursor_trail),
            "cursor_ripples" => parse_input!(bool: cursor_ripples),
            "cursor_scale_to_cs" => parse_input!(bool: cursor_scale_to_cs),
            "intro_bg_dim" => parse_input!(percent: intro_bg_dim),
            "ingame_bg_dim" => parse_input!(percent: ingame_bg_dim),
            "break_bg_dim" => parse_input!(percent: break_bg_dim),
            "bg_parallax" => parse_input!(bool: bg_parallax),
            "load_storyboard" => parse_input!(bool: load_storyboard),
            "load_video" => parse_input!(bool: load_video),
            "skip_intro" => parse_input!(bool: skip_intro),
            "show_danser_logo" => parse_input!(bool: show_danser_logo),
            "seizure_warning" => parse_input!(bool: seizure_warning),
            "objects_rainbow" => parse_input!(bool: objects_rainbow),
            "flash_objects" => parse_input!(bool: flash_objects),
            "slider_merge" => parse_input!(bool: slider_merge),
            "slider_snaking_in" => parse_input!(bool: slider_snaking_in),
            "slider_snaking_out" => parse_input!(bool: slider_snaking_out),
            "use_slider_hitcircle_color" => parse_input!(bool: use_slider_hitcircle_color),
            "draw_combo_numbers" => parse_input!(bool: draw_combo_numbers),
            "beat_scaling" => parse_input!(bool: beat_scaling),
            "use_beatmap_colors" => parse_input!(bool: use_beatmap_colors),
            "draw_follow_points" => parse_input!(bool: draw_follow_points),
            other => bail!("Unknown settings modal `{other}`"),
        }

        if !deferred {
            if let Err(err) = modal.defer(ctx).await {
                warn!("Failed to defer modal");
            }
        }

        let res = ctx
            .replay()
            .set_settings(self.msg_owner, &self.skin, &self.settings)
            .await;

        self.defer_next = res.is_ok();

        res
    }
}

impl IActiveMessage for RenderSettingsActive {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let Self {
            skin,
            settings,
            group,
            content,
            ..
        } = self;

        let embed = EmbedBuilder::new()
            .title(group.title())
            .description(group.description(skin, settings, self.skin_status.take()));

        BuildPage::new(embed, mem::replace(&mut self.defer_next, false))
            .content(content.take().unwrap_or_default())
            .boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        let group_options = vec![
            SelectMenuOption {
                default: false,
                description: None,
                emoji: None,
                label: "Skin".to_owned(),
                value: "skin".to_owned(),
            },
            SelectMenuOption {
                default: false,
                description: None,
                emoji: None,
                label: "Audio".to_owned(),
                value: "audio".to_owned(),
            },
            SelectMenuOption {
                default: false,
                description: None,
                emoji: None,
                label: "HUD".to_owned(),
                value: "hud".to_owned(),
            },
            SelectMenuOption {
                default: false,
                description: None,
                emoji: None,
                label: "Cursor".to_owned(),
                value: "cursor".to_owned(),
            },
            SelectMenuOption {
                default: false,
                description: None,
                emoji: None,
                label: "Background".to_owned(),
                value: "background".to_owned(),
            },
            SelectMenuOption {
                default: false,
                description: None,
                emoji: None,
                label: "Intro".to_owned(),
                value: "intro".to_owned(),
            },
            SelectMenuOption {
                default: false,
                description: None,
                emoji: None,
                label: "Objects".to_owned(),
                value: "objects".to_owned(),
            },
        ];

        let group = SelectMenu {
            custom_id: "group_menu".to_owned(),
            disabled: false,
            max_values: None,
            min_values: None,
            options: group_options,
            placeholder: Some("Select a settings group".to_owned()),
        };

        let edit_options = self.group.edit_options();

        let edit = SelectMenu {
            custom_id: "edit_menu".to_owned(),
            disabled: false,
            max_values: None,
            min_values: None,
            options: edit_options,
            placeholder: Some("Select a value to modify from this group".to_owned()),
        };

        let group_menu = ActionRow {
            components: vec![Component::SelectMenu(group)],
        };

        let edit_menu = ActionRow {
            components: vec![Component::SelectMenu(edit)],
        };

        vec![
            Component::ActionRow(group_menu),
            Component::ActionRow(edit_menu),
        ]
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err).boxed(),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore.boxed();
        }

        match component.data.custom_id.as_str() {
            "group_menu" => Box::pin(self.handle_group_menu(ctx, component)),
            "edit_menu" => Box::pin(self.handle_edit_menu(ctx, component)),
            other => ComponentResult::Err(eyre!("Unknown settings component `{other}`")).boxed(),
        }
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(self.async_handle_modal(ctx, modal))
    }
}

#[derive(Copy, Clone, Default)]
enum SettingsGroup {
    #[default]
    Skin,
    Audio,
    Hud,
    Cursor,
    Background,
    Intro,
    Objects,
}

impl SettingsGroup {
    fn title(self) -> String {
        let kind = match self {
            SettingsGroup::Skin => "Skin",
            SettingsGroup::Audio => "Audio",
            SettingsGroup::Hud => "HUD",
            SettingsGroup::Cursor => "Cursor",
            SettingsGroup::Background => "Background",
            SettingsGroup::Intro => "Intro",
            SettingsGroup::Objects => "Objects",
        };

        format!("Your current render settings for `{kind}`:")
    }

    fn description(
        self,
        skin: &RenderSkinOption<'_>,
        settings: &RenderOptions,
        skin_status: SkinStatus,
    ) -> String {
        match self {
            SettingsGroup::Skin => format!(
                "{skin_status}- Skin: `{}`\n\
                - Use skin cursor: `{}`\n\
                - Use skin hitsounds: `{}`\n\
                \n\
                Check out [the website](https://ordr.issou.best/skins) to see all available skins",
                skin.skin_name, settings.use_skin_cursor, settings.use_skin_hitsounds,
            ),
            SettingsGroup::Audio => format!(
                "- Global volume: `{}`\n\
                - Music volume: `{}`\n\
                - Hitsound volume: `{}`\n\
                - Play nightcore samples: `{}`",
                settings.global_volume,
                settings.music_volume,
                settings.hitsound_volume,
                settings.play_nightcore_samples,
            ),
            SettingsGroup::Hud => format!(
                "- Show hit error meter: `{}`\n\
                - Show aim error meter: `{}`\n\
                - Show HP bar: `{}`\n\
                - Show key overlay: `{}`\n\
                - Show borders: `{}`\n\
                - Show mods: `{}`\n\
                - Show score: `{}`\n\
                - Show combo counter: `{}`\n\
                - Show pp counter: `{}`\n\
                - Show hit counter: `{}`\n\
                - Show unstable rate: `{}`\n\
                - Show scoreboard: `{}`\n\
                - Show avatars on scoreboard: `{}`",
                settings.show_hit_error_meter,
                settings.show_aim_error_meter,
                settings.show_hp_bar,
                settings.show_key_overlay,
                settings.show_borders,
                settings.show_mods,
                settings.show_score,
                settings.show_combo_counter,
                settings.show_pp_counter,
                settings.show_hit_counter,
                settings.show_unstable_rate,
                settings.show_scoreboard,
                settings.show_avatars_on_scoreboard,
            ),
            SettingsGroup::Cursor => format!(
                "- Cursor rainbow: `{}`\n\
                - Cursor trail glow: `{}`\n\
                - Cursor size: `{}`\n\
                - Cursor trail: `{}`\n\
                - Cursor ripples: `{}`\n\
                - Cursor scale to CS: `{}`",
                settings.cursor_rainbow,
                settings.cursor_trail_glow,
                round(settings.cursor_size),
                settings.cursor_trail,
                settings.cursor_ripples,
                settings.cursor_scale_to_cs,
            ),
            SettingsGroup::Background => format!(
                "- Intro BG dim: `{}`\n\
                - Ingame BG dim: `{}`\n\
                - Break BG dim: `{}`\n\
                - BG parallax: `{}`\n\
                - Load storyboard: `{}`\n\
                - Load video: `{}`",
                settings.intro_bg_dim,
                settings.ingame_bg_dim,
                settings.break_bg_dim,
                settings.bg_parallax,
                settings.load_storyboard,
                settings.load_video,
            ),
            SettingsGroup::Intro => format!(
                "- Intro BG dim: `{}`\n\
                - Skip intro: `{}`\n\
                - Show danser logo: `{}`\n\
                - Seizure warning: `{}`",
                settings.intro_bg_dim,
                settings.skip_intro,
                settings.show_danser_logo,
                settings.seizure_warning,
            ),
            SettingsGroup::Objects => format!(
                "- Object rainbow: `{}`\n\
                - Flash objects: `{}`\n\
                - Slider merge: `{}`\n\
                - Slider snaking in: `{}`\n\
                - Slider snaking out: `{}`\n\
                - Use slider hitcircle color: `{}`\n\
                - Draw combo numbers: `{}`\n\
                - Beat scaling: `{}`\n\
                - Use beatmap colors: `{}`\n\
                - Draw follow points: `{}`",
                settings.objects_rainbow,
                settings.flash_objects,
                settings.slider_merge,
                settings.slider_snaking_in,
                settings.slider_snaking_out,
                settings.use_slider_hitcircle_color,
                settings.draw_combo_numbers,
                settings.beat_scaling,
                settings.use_beatmap_colors,
                settings.draw_follow_points,
            ),
        }
    }

    fn edit_options(self) -> Vec<SelectMenuOption> {
        match self {
            SettingsGroup::Skin => vec![
                SelectMenuOption {
                    default: false,
                    description: Some(
                        "The name of the skin to use".to_owned(),
                    ),
                    emoji: None,
                    label: "Skin".to_owned(),
                    value: "skin".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some(
                        "Use the skin cursor (if false danser cursor will be used)".to_owned(),
                    ),
                    emoji: None,
                    label: "Use skin cursor".to_owned(),
                    value: "use_skin_cursor".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some(
                        "Use skin hitsounds (if false beatmap hitsounds will be used)".to_owned(),
                    ),
                    emoji: None,
                    label: "Use skin hitsounds".to_owned(),
                    value: "use_skin_hitsounds".to_owned(),
                },
            ],
            SettingsGroup::Audio => vec![
                SelectMenuOption {
                    default: false,
                    description: Some("The global volume for the video".to_owned()),
                    emoji: None,
                    label: "Global volume".to_owned(),
                    value: "global_volume".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("The music volume for the video".to_owned()),
                    emoji: None,
                    label: "Music volume".to_owned(),
                    value: "music_volume".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("The hitsounds volume for the video".to_owned()),
                    emoji: None,
                    label: "Hitsound volume".to_owned(),
                    value: "hitsound_volume".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Play nightcore hitsounds or not".to_owned()),
                    emoji: None,
                    label: "Play nightcore samples".to_owned(),
                    value: "play_nightcore_samples".to_owned(),
                },
            ],
            SettingsGroup::Hud => vec![
                SelectMenuOption {
                    default: false,
                    description: Some("Show the hit error meter".to_owned()),
                    emoji: None,
                    label: "Show hit error meter".to_owned(),
                    value: "show_hit_error_meter".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the aim error meter".to_owned()),
                    emoji: None,
                    label: "Show aim error meter".to_owned(),
                    value: "show_aim_error_meter".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the HP bar".to_owned()),
                    emoji: None,
                    label: "Show HP bar".to_owned(),
                    value: "show_hp_bar".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the key overlay".to_owned()),
                    emoji: None,
                    label: "Show key overlay".to_owned(),
                    value: "show_key_overlay".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the playfield borders or not".to_owned()),
                    emoji: None,
                    label: "Show borders".to_owned(),
                    value: "show_borders".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the mods used during the game or not".to_owned()),
                    emoji: None,
                    label: "Show mods".to_owned(),
                    value: "show_mods".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the score".to_owned()),
                    emoji: None,
                    label: "Show score".to_owned(),
                    value: "show_score".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the combo counter".to_owned()),
                    emoji: None,
                    label: "Show combo counter".to_owned(),
                    value: "show_combo_counter".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the pp counter or not".to_owned()),
                    emoji: None,
                    label: "Show pp counter".to_owned(),
                    value: "show_pp_counter".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Shows a hit counter (100, 50, miss) below the PP counter".to_owned()),
                    emoji: None,
                    label: "Show hit counter".to_owned(),
                    value: "show_hit_counter".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the unstable rate (only takes effect if 'Show hit error meter' is set to true)".to_owned()),
                    emoji: None,
                    label: "Show unstable rate".to_owned(),
                    value: "show_unstable_rate".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the scoreboard or not".to_owned()),
                    emoji: None,
                    label: "Show scoreboard".to_owned(),
                    value: "show_scoreboard".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show avatars on the left of the username of a player on the scoreboard".to_owned()),
                    emoji: None,
                    label: "Show avatars on scoreboard".to_owned(),
                    value: "show_avatars_on_scoreboard".to_owned(),
                },
            ],
            SettingsGroup::Cursor => vec![
                SelectMenuOption {
                    default: false,
                    description: Some("Makes the cursor rainbow (only takes effect if 'Use skin cursor' is set to false)".to_owned()),
                    emoji: None,
                    label: "Cursor rainbow".to_owned(),
                    value: "cursor_rainbow".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Have a glow with the trail or not".to_owned()),
                    emoji: None,
                    label: "Cursor trail glow".to_owned(),
                    value: "cursor_trail_glow".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Set the cursor size".to_owned()),
                    emoji: None,
                    label: "Cursor size".to_owned(),
                    value: "cursor_size".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the cursor trail or not".to_owned()),
                    emoji: None,
                    label: "Cursor trail".to_owned(),
                    value: "cursor_trail".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show cursor ripples on keypress".to_owned()),
                    emoji: None,
                    label: "Cursor ripples".to_owned(),
                    value: "cursor_ripples".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Scale cursor to circle size".to_owned()),
                    emoji: None,
                    label: "Cursor scale to CS".to_owned(),
                    value: "cursor_scale_to_cs".to_owned(),
                },
            ],
            SettingsGroup::Background => vec![
                SelectMenuOption {
                    default: false,
                    description: Some("Background dim for the intro".to_owned()),
                    emoji: None,
                    label: "Intro BG dim".to_owned(),
                    value: "intro_bg_dim".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Background dim in game".to_owned()),
                    emoji: None,
                    label: "Ingame BG dim".to_owned(),
                    value: "ingame_bg_dim".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Background dim during breaks".to_owned()),
                    emoji: None,
                    label: "Break BG dim".to_owned(),
                    value: "break_bg_dim".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Adds a parallax effect".to_owned()),
                    emoji: None,
                    label: "BG parallax".to_owned(),
                    value: "bg_parallax".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Load the background storyboard".to_owned()),
                    emoji: None,
                    label: "Load storyboard".to_owned(),
                    value: "load_storyboard".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Load the background video ('Load storyboard' has to be set to true)".to_owned()),
                    emoji: None,
                    label: "Load video".to_owned(),
                    value: "load_video".to_owned(),
                },
            ],
            SettingsGroup::Intro => vec![
                SelectMenuOption {
                    default: false,
                    description: Some("Background dim for the intro".to_owned()),
                    emoji: None,
                    label: "Intro BG dim".to_owned(),
                    value: "intro_bg_dim".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Skip the intro or not".to_owned()),
                    emoji: None,
                    label: "Skip intro".to_owned(),
                    value: "skip_intro".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show danser logo on the intro".to_owned()),
                    emoji: None,
                    label: "Show danser logo".to_owned(),
                    value: "show_danser_logo".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Display a 5 second seizure warning before the video".to_owned()),
                    emoji: None,
                    label: "Seizure warning".to_owned(),
                    value: "seizure_warning".to_owned(),
                },
            ],
            SettingsGroup::Objects => vec![
                SelectMenuOption {
                    default: false,
                    description: Some("Makes the objects rainbow (overrides 'Use skin colors' and 'Use beatmap colors')".to_owned()),
                    emoji: None,
                    label: "Objects rainbow".to_owned(),
                    value: "objects_rainbow".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Makes the objects flash to the beat".to_owned()),
                    emoji: None,
                    label: "Flash objects".to_owned(),
                    value: "flash_objects".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Merge sliders or not".to_owned()),
                    emoji: None,
                    label: "Slider merge".to_owned(),
                    value: "slider_merge".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Have slider snaking in".to_owned()),
                    emoji: None,
                    label: "Slider snaking in".to_owned(),
                    value: "slider_snaking_in".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Have slider snaking out".to_owned()),
                    emoji: None,
                    label: "Slider snaking out".to_owned(),
                    value: "slider_snaking_out".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Makes the slider body have the same color as the hit circles".to_owned()),
                    emoji: None,
                    label: "Use slider hitcircle color".to_owned(),
                    value: "use_slider_hitcircle_color".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Show the combo numbers in objects".to_owned()),
                    emoji: None,
                    label: "Draw combo numbers".to_owned(),
                    value: "draw_combo_numbers".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Scale objects to the beat".to_owned()),
                    emoji: None,
                    label: "Beat scaling".to_owned(),
                    value: "beat_scaling".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Use the beatmap combo colors (overrides 'Use skin colors' if set to true)".to_owned()),
                    emoji: None,
                    label: "Use beatmap colors".to_owned(),
                    value: "use_beatmap_colors".to_owned(),
                },
                SelectMenuOption {
                    default: false,
                    description: Some("Draw follow points between objects or not".to_owned()),
                    emoji: None,
                    label: "Draw follow points".to_owned(),
                    value: "draw_follow_points".to_owned(),
                },
            ],
        }
    }
}

#[derive(Copy, Clone, Default)]
enum SkinStatus {
    #[default]
    Ok,
    NotFound,
    Err,
}

impl SkinStatus {
    fn take(&mut self) -> Self {
        mem::replace(self, Self::Ok)
    }
}

impl Display for SkinStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            SkinStatus::Ok => Ok(()),
            SkinStatus::NotFound => f.write_str("⚠️ No skin found for your last input\n"),
            SkinStatus::Err => f.write_str("⚠️ Failed to validate skin, maybe try again later\n"),
        }
    }
}

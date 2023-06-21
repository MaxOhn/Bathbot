use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    ptr,
    sync::Arc,
};

use bathbot_util::{
    modal::{ModalBuilder, TextInputBuilder},
    EmbedBuilder, FooterBuilder,
};
use eyre::{Report, Result, WrapErr};
use futures::future::BoxFuture;
use rosu_render::model::{RenderOptions, RenderSkinOption};
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle, TextInputStyle},
        Component, ReactionType,
    },
    id::{marker::UserMarker, Id},
};

use super::RenderSettingsActive;
use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    core::Context,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Authored, ModalExt,
    },
};

#[derive(Default)]
enum ImportResult {
    #[default]
    None,
    OkWithDefer(RenderSettingsActive),
    Ok(RenderSettingsActive),
    ParseError(ParseError),
    Err(Report),
}

impl ImportResult {
    /// If `self` is `Self::OkWithDefer`, replace it with `Self::Ok` and return
    /// `true`. Otherwise do nothing and return `false`.
    fn skip_defer(&mut self) -> bool {
        if !matches!(self, Self::OkWithDefer(_)) {
            return false;
        }

        debug_assert!(matches!(self, Self::OkWithDefer(_)));

        // SAFETY: self is valid for reads, properly aligned, and initialized
        let tmp = unsafe { ptr::read(self) };

        // Code must not panic between ptr::read and ptr::write

        let new = match tmp {
            Self::OkWithDefer(s) => Self::Ok(s),
            _ => unreachable!(), // previous assert ensures that this can not happen
        };

        // SAFETY: self is valid for writes, and properly aligned
        unsafe { ptr::write(self, new) };

        true
    }
}

pub struct SettingsImport {
    msg_owner: Id<UserMarker>,
    import_result: ImportResult,
}

impl SettingsImport {
    pub fn new(msg_owner: Id<UserMarker>) -> Self {
        Self {
            msg_owner,
            import_result: Default::default(),
        }
    }

    async fn async_handle_modal(
        &mut self,
        ctx: &Context,
        modal: &mut InteractionModal,
    ) -> Result<()> {
        #[cfg(debug_assertions)]
        ensure!(
            modal.data.custom_id == "import",
            "Unexpected setting import modal `{}`",
            modal.data.custom_id
        );

        let input_opt = modal
            .data
            .components
            .pop()
            .and_then(|mut row| row.components.pop())
            .and_then(|component| component.value);

        let Some(input) = input_opt else {
            return Err(eyre!("Missing settings import modal input"));
        };

        modal.defer(ctx).await.wrap_err("Failed to defer modal")?;

        self.import_result = match parse(&input) {
            Ok((settings, skin)) => {
                let ordr = ctx.ordr().client();

                match ordr.skin_list().search(skin.skin_name.as_ref()).await {
                    Ok(skin_list) if !skin_list.skins.is_empty() => {
                        let user = modal.user_id()?;

                        match ctx.replay().set_settings(user, &skin, &settings).await {
                            Ok(_) => {
                                let skin = RenderSkinOption::<'static> {
                                    skin_name: skin.skin_name.into_owned().into(),
                                    ..skin
                                };

                                let active = RenderSettingsActive::new(
                                    skin,
                                    settings,
                                    Some("Successfully imported settings"),
                                    self.msg_owner,
                                );

                                ImportResult::OkWithDefer(active)
                            }
                            Err(err) => ImportResult::Err(err),
                        }
                    }
                    Ok(_) => {
                        ImportResult::ParseError(ParseError::InvalidValue(Setting::DefaultSkin))
                    }
                    Err(err) => {
                        ImportResult::Err(Report::new(err).wrap_err("Failed to request skin"))
                    }
                }
            }
            Err(err) => ImportResult::ParseError(err),
        };

        Ok(())
    }
}

impl IActiveMessage for SettingsImport {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        const TITLE: &str = "Copy Yuna's settings, click the button, and paste them in";
        const IMAGE_URL: &str = "https://media.discordapp.net/attachments/579428622964621324/1120820561443029154/image.png";

        let skipped_defer = self.import_result.skip_defer();

        let (embed, defer) = match self.import_result {
            ImportResult::None => (EmbedBuilder::new().title(TITLE).image(IMAGE_URL), false),
            ImportResult::Ok(ref mut active) => {
                if skipped_defer {
                    let fut = async {
                        match active.build_page(ctx).await {
                            Ok(mut build) => {
                                build.defer = true;

                                Ok(build)
                            }
                            err @ Err(_) => err,
                        }
                    };

                    return Box::pin(fut);
                } else {
                    return active.build_page(ctx);
                }
            }
            ImportResult::ParseError(ref err) => {
                let footer = match err {
                    ParseError::InsufficientLineCount => {
                        "Error: Expected more lines, did you copy-paste everything?".to_owned()
                    }
                    ParseError::Missing(setting) => {
                        format!("Error: Missing `{setting}`, did you copy-paste everything?")
                    }
                    ParseError::InvalidValue(setting) => {
                        format!("Error: Invalid value for `{setting}`")
                    }
                };

                let embed = EmbedBuilder::new()
                    .title(TITLE)
                    .image(IMAGE_URL)
                    .color_red()
                    .footer(FooterBuilder::new(footer));

                (embed, true)
            }
            ImportResult::Err(ref err) => {
                warn!(?err, "Import result error");

                let embed = EmbedBuilder::new()
                    .color_red()
                    .description("Something went wrong, try again later");

                (embed, true)
            }
            ImportResult::OkWithDefer(_) => unreachable!(),
        };

        BuildPage::new(embed, defer).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        match &self.import_result {
            ImportResult::None | ImportResult::ParseError(_) => {
                let import = Button {
                    custom_id: Some("import".to_owned()),
                    disabled: false,
                    emoji: Some(ReactionType::Unicode {
                        name: "📋".to_owned(),
                    }),
                    label: Some("Paste settings".to_owned()),
                    style: ButtonStyle::Success,
                    url: None,
                };

                let row = ActionRow {
                    components: vec![Component::Button(import)],
                };

                vec![Component::ActionRow(row)]
            }
            ImportResult::OkWithDefer(active) | ImportResult::Ok(active) => {
                active.build_components()
            }
            ImportResult::Err(_) => Vec::new(),
        }
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        if let ImportResult::OkWithDefer(active) | ImportResult::Ok(active) =
            &mut self.import_result
        {
            return active.handle_component(ctx, component);
        }

        #[cfg(debug_assertions)]
        if component.data.custom_id != "import" {
            return Box::pin(std::future::ready(ComponentResult::Err(eyre!(
                "Unexpected setting import component `{}`",
                component.data.custom_id
            ))));
        }

        let owner = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err).boxed(),
        };

        if owner != self.msg_owner {
            return ComponentResult::Ignore.boxed();
        }

        let input = TextInputBuilder::new("input", "Yuna embed text")
            .placeholder("Copy-paste Yuna's settings embed")
            .style(TextInputStyle::Paragraph)
            .required(true);

        let modal = ModalBuilder::new("import", "Import render settings from Yuna").input(input);

        ComponentResult::CreateModal(modal).boxed()
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        if let ImportResult::OkWithDefer(ref mut active) | ImportResult::Ok(ref mut active) =
            self.import_result
        {
            return active.handle_modal(ctx, modal);
        }

        Box::pin(self.async_handle_modal(ctx, modal))
    }
}

enum ParseError {
    InsufficientLineCount,
    Missing(Setting),
    InvalidValue(Setting),
}

#[derive(Copy, Clone)]
enum Setting {
    DefaultSkin,
    MusicVolume,
    HitsoundsVolume,
    UseSkinCursor,
    ComboColors,
    ShowPpCounter,
    ShowScoreboard,
    ShowHitCounter,
    ShowAimErrorMeter,
    IntroDim,
    IngameDim,
    BreakDim,
    SliderSnakingIn,
    SliderSnakingOut,
    SkinHitsounds,
    CursorSize,
    Skip,
    LoadVideoStoryboard,
}

impl Setting {
    fn as_str(self) -> &'static str {
        match self {
            Self::DefaultSkin => "Default skin",
            Self::MusicVolume => "Music volume",
            Self::HitsoundsVolume => "Hitsounds volume",
            Self::UseSkinCursor => "Use skin cursor",
            Self::ComboColors => "Combo colors",
            Self::ShowPpCounter => "Show PP Counter",
            Self::ShowScoreboard => "Show Scoreboard",
            Self::ShowHitCounter => "Show Hit Counter",
            Self::ShowAimErrorMeter => "Show Aim Error Meter",
            Self::IntroDim => "Intro Dim",
            Self::IngameDim => "In-game Dim",
            Self::BreakDim => "Break Dim",
            Self::SliderSnakingIn => "Slider snaking in",
            Self::SliderSnakingOut => "Slider snaking out",
            Self::SkinHitsounds => "Skin hitsounds",
            Self::CursorSize => "Cursor size",
            Self::Skip => "Skip",
            Self::LoadVideoStoryboard => "Load video/storyboard",
        }
    }
}

impl Display for Setting {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_str())
    }
}

fn parse(mut input: &str) -> Result<(RenderOptions, RenderSkinOption<'_>), ParseError> {
    let start = input
        .find("Default skin:")
        .ok_or(ParseError::Missing(Setting::DefaultSkin))?;

    input = &input[start..];

    let mut lines = input.lines();

    let mut get_line = |setting: Setting| {
        lines
            .next()
            .ok_or(ParseError::InsufficientLineCount)?
            .strip_prefix(setting.as_str())
            .and_then(|line| line.strip_prefix(": "))
            .ok_or(ParseError::Missing(setting))
    };

    let skin = get_line(Setting::DefaultSkin)?;
    let music_volume = get_line(Setting::MusicVolume)?;
    let hitsounds_volume = get_line(Setting::HitsoundsVolume)?;
    let use_skin_cursor = get_line(Setting::UseSkinCursor)?;
    let combo_colors = get_line(Setting::ComboColors)?;
    let show_pp_counter = get_line(Setting::ShowPpCounter)?;
    let show_scoreboard = get_line(Setting::ShowScoreboard)?;
    let show_hit_counter = get_line(Setting::ShowHitCounter)?;
    let show_aim_error_meter = get_line(Setting::ShowAimErrorMeter)?;
    let intro_dim = get_line(Setting::IntroDim)?;
    let ingame_dim = get_line(Setting::IngameDim)?;
    let break_dim = get_line(Setting::BreakDim)?;
    let slider_snaking_in = get_line(Setting::SliderSnakingIn)?;
    let slider_snaking_out = get_line(Setting::SliderSnakingOut)?;
    let skin_hitsounds = get_line(Setting::SkinHitsounds)?;
    let cursor_size = get_line(Setting::CursorSize)?;
    let skip = get_line(Setting::Skip)?;
    let load_video_storyboard = get_line(Setting::LoadVideoStoryboard)?;

    fn parse_percent(input: &str) -> Option<u8> {
        input.strip_suffix('%')?.parse().ok()
    }

    fn parse_bool(input: &str) -> Option<bool> {
        match input {
            ":white_check_mark:" => Some(true),
            ":x:" => Some(false),
            _ => None,
        }
    }

    let (use_slider_hitcircle_color, use_beatmap_colors) = match combo_colors {
        "beatmap" => (false, true),
        "skin" => (true, false),
        _ => return Err(ParseError::InvalidValue(Setting::ComboColors)),
    };

    let video_storyboard = parse_bool(load_video_storyboard)
        .ok_or(ParseError::InvalidValue(Setting::LoadVideoStoryboard))?;

    let skin = RenderSkinOption::new(skin, true);

    let options = RenderOptions {
        music_volume: parse_percent(music_volume)
            .ok_or(ParseError::InvalidValue(Setting::MusicVolume))?,
        hitsound_volume: parse_percent(hitsounds_volume)
            .ok_or(ParseError::InvalidValue(Setting::HitsoundsVolume))?,
        use_skin_cursor: parse_bool(use_skin_cursor)
            .ok_or(ParseError::InvalidValue(Setting::UseSkinCursor))?,
        show_pp_counter: parse_bool(show_pp_counter)
            .ok_or(ParseError::InvalidValue(Setting::ShowPpCounter))?,
        show_scoreboard: parse_bool(show_scoreboard)
            .ok_or(ParseError::InvalidValue(Setting::ShowScoreboard))?,
        show_hit_counter: parse_bool(show_hit_counter)
            .ok_or(ParseError::InvalidValue(Setting::ShowHitCounter))?,
        show_aim_error_meter: parse_bool(show_aim_error_meter)
            .ok_or(ParseError::InvalidValue(Setting::ShowAimErrorMeter))?,
        intro_bg_dim: parse_percent(intro_dim)
            .ok_or(ParseError::InvalidValue(Setting::IntroDim))?,
        ingame_bg_dim: parse_percent(ingame_dim)
            .ok_or(ParseError::InvalidValue(Setting::IngameDim))?,
        break_bg_dim: parse_percent(break_dim)
            .ok_or(ParseError::InvalidValue(Setting::BreakDim))?,
        slider_snaking_in: parse_bool(slider_snaking_in)
            .ok_or(ParseError::InvalidValue(Setting::SliderSnakingIn))?,
        slider_snaking_out: parse_bool(slider_snaking_out)
            .ok_or(ParseError::InvalidValue(Setting::SliderSnakingOut))?,
        use_skin_hitsounds: parse_bool(skin_hitsounds)
            .ok_or(ParseError::InvalidValue(Setting::UseSkinCursor))?,
        cursor_size: cursor_size
            .strip_suffix('x')
            .and_then(|line| line.parse().ok())
            .ok_or(ParseError::InvalidValue(Setting::CursorSize))?,
        skip_intro: parse_bool(skip).ok_or(ParseError::InvalidValue(Setting::Skip))?,
        load_video: video_storyboard,
        load_storyboard: video_storyboard,
        use_beatmap_colors,
        use_slider_hitcircle_color,
        ..Default::default()
    };

    Ok((options, skin))
}

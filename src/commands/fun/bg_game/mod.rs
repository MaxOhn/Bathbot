#![allow(non_upper_case_globals)]

mod bigger;
mod hint;
mod rankings;
mod skip;
mod stop;
mod tags;

use std::sync::Arc;

use bitflags::bitflags;
use dashmap::mapref::entry::Entry;
use eyre::Report;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        component::{
            button::ButtonStyle, select_menu::SelectMenuOption, ActionRow, Button, Component,
            SelectMenu,
        },
        interaction::{
            application_command::CommandOptionValue, ApplicationCommand,
            MessageComponentInteraction,
        },
    },
    channel::{
        embed::{Embed, EmbedField},
        Reaction,
    },
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    id::{marker::UserMarker, Id},
};

use crate::{
    bg_game::{GameWrapper, MapsetTags},
    commands::{parse_mode_option, MyCommand, MyCommandOption},
    embeds::{BGTagsEmbed, EmbedBuilder, EmbedData},
    error::{Error, InvalidBgState},
    util::{
        constants::{
            common_literals::{HELP, MANIA, MODE, OSU, SPECIFY_MODE},
            GENERAL_ISSUE, RED,
        },
        InteractionExt, MessageBuilder, MessageExt,
    },
    BotResult, CommandData, Context,
};

pub use self::{bigger::*, hint::*, rankings::*, skip::*, stop::*, tags::*};

#[command]
#[short_desc("Play the background guessing game, use `/bg` to start")]
#[aliases("bg")]
#[sub_commands(skip, bigger, hint, stop, rankings)]
pub async fn backgroundgame(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, .. } => match args.next() {
            None | Some(HELP) => {
                let content = "Use `/bg` to start a new background guessing game.\n\
                    Given part of a map's background, try to guess the **title** of the map's song.\n\
                    You don't need to guess content in parentheses `(...)` or content after `ft.` or `feat.`.\n\n\
                    Use these prefix commands to initiate with the game:\n\
                    • `<bg s[kip]` / `<bg r[esolve]`: Resolve the current background and \
                    give a new one with the same tag specs.\n\
                    • `<bg h[int]`: Receive a hint (can be used multiple times).\n\
                    • `<bg b[igger]`: Increase the radius of the displayed image (can be used multiple times).\n\
                    • `<bg stop`: Resolve the current background and stop the game.
                    • `<bg l[eaderboard] s[erver]`: Check out the global leaderboard for \
                    amount of correct guesses. If `server` or `s` is added at the end, \
                    I will only show members of this server.";

                let builder = MessageBuilder::new().embed(content);
                msg.create_message(&ctx, builder).await?;

                Ok(())
            }
            _ => {
                let prefix = ctx.guild_first_prefix(msg.guild_id).await;

                let content =
                    format!("That's not a valid subcommand. Check `{prefix}bg` for more help.");

                msg.error(&ctx, content).await
            }
        },
        CommandData::Interaction { .. } => unreachable!(),
    }
}

enum ReactionWrapper {
    Add(Reaction),
    Remove(Reaction),
}

impl ReactionWrapper {
    fn as_deref(&self) -> &Reaction {
        match self {
            Self::Add(r) | Self::Remove(r) => r,
        }
    }
}

bitflags! {
    pub struct Effects: u8 {
        const Blur           = 1 << 0;
        const Contrast       = 1 << 1;
        const FlipHorizontal = 1 << 2;
        const FlipVertical   = 1 << 3;
        const Grayscale      = 1 << 4;
        const Invert         = 1 << 5;
    }
}

#[derive(Copy, Clone, Debug)]
pub enum GameDifficulty {
    Normal,
    Hard,
    Impossible,
}

impl GameDifficulty {
    pub fn value(self) -> f32 {
        match self {
            GameDifficulty::Normal => 0.5,
            GameDifficulty::Hard => 0.75,
            GameDifficulty::Impossible => 0.95,
        }
    }
}

impl Default for GameDifficulty {
    fn default() -> Self {
        Self::Normal
    }
}

pub enum BgGameState {
    Running {
        game: GameWrapper,
    },
    Setup {
        author: Id<UserMarker>,
        difficulty: GameDifficulty,
        effects: Effects,
        excluded: MapsetTags,
        included: MapsetTags,
    },
}

fn parse_component_tags(component: &MessageComponentInteraction) -> MapsetTags {
    component
        .data
        .values
        .iter()
        .fold(MapsetTags::empty(), |tags, value| {
            tags | match value.as_str() {
                "easy" => MapsetTags::Easy,
                "hard" => MapsetTags::Hard,
                "meme" => MapsetTags::Meme,
                "weeb" => MapsetTags::Weeb,
                "kpop" => MapsetTags::Kpop,
                "farm" => MapsetTags::Farm,
                "hardname" => MapsetTags::HardName,
                "alt" => MapsetTags::Alternate,
                "bluesky" => MapsetTags::BlueSky,
                "english" => MapsetTags::English,
                "streams" => MapsetTags::Streams,
                "old" => MapsetTags::Old,
                "tech" => MapsetTags::Tech,
                _ => {
                    warn!("unknown mapset tag `{value}`");

                    return tags;
                }
            }
        })
}

async fn update_field(
    ctx: &Context,
    component: &mut MessageComponentInteraction,
    tags: MapsetTags,
    name: &str,
) -> BotResult<()> {
    let mut embed = component
        .message
        .embeds
        .pop()
        .ok_or(InvalidBgState::MissingEmbed)?;

    let field_opt = embed.fields.iter_mut().find(|field| field.name == name);

    if let Some(field) = field_opt {
        field.value = tags.join(", ");
    } else {
        let field = EmbedField {
            inline: false,
            name: name.to_owned(),
            value: tags.join(", "),
        };

        embed.fields.push(field);
    }

    let data = InteractionResponseData {
        embeds: Some(vec![embed]),
        ..Default::default()
    };

    let response = InteractionResponse {
        kind: InteractionResponseType::UpdateMessage,
        data: Some(data),
    };

    let client = ctx.interaction();

    client
        .create_response(component.id, &component.token, &response)
        .exec()
        .await?;

    Ok(())
}

pub async fn handle_bg_start_include(
    ctx: &Context,
    mut component: MessageComponentInteraction,
) -> BotResult<()> {
    match ctx.bg_games().entry(component.channel_id) {
        Entry::Occupied(mut entry) => match entry.get_mut() {
            BgGameState::Running { .. } => {
                if let Err(err) = remove_components(ctx, &component, None).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }
            }
            BgGameState::Setup {
                author, included, ..
            } => {
                if *author != component.user_id()? {
                    return Ok(());
                }

                *included = parse_component_tags(&component);
                update_field(ctx, &mut component, *included, "Included tags").await?;
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(ctx, &component, None).await {
                let report = Report::new(err).wrap_err("failed to remove components");
                warn!("{report:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_exclude(
    ctx: &Context,
    mut component: MessageComponentInteraction,
) -> BotResult<()> {
    match ctx.bg_games().entry(component.channel_id) {
        Entry::Occupied(mut entry) => match entry.get_mut() {
            BgGameState::Running { .. } => {
                if let Err(err) = remove_components(ctx, &component, None).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }
            }
            BgGameState::Setup {
                author, excluded, ..
            } => {
                if *author != component.user_id()? {
                    return Ok(());
                }

                *excluded = parse_component_tags(&component);
                update_field(ctx, &mut component, *excluded, "Excluded tags").await?;
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(ctx, &component, None).await {
                let report = Report::new(err).wrap_err("failed to remove components");
                warn!("{report:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_button(
    ctx: Arc<Context>,
    component: MessageComponentInteraction,
) -> BotResult<()> {
    let channel = component.channel_id;

    match ctx.bg_games().entry(channel) {
        Entry::Occupied(mut entry) => match entry.get() {
            BgGameState::Running { .. } => {
                if let Err(err) = remove_components(&ctx, &component, None).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }
            }
            BgGameState::Setup {
                author,
                difficulty,
                effects,
                excluded,
                included,
            } => {
                if *author != component.user_id()? {
                    return Ok(());
                }

                let mapset_fut =
                    ctx.psql()
                        .get_specific_tags_mapset(GameMode::STD, *included, *excluded);

                let mapsets = match mapset_fut.await {
                    Ok(mapsets) => mapsets,
                    Err(err) => {
                        let embed = EmbedBuilder::new()
                            .color(RED)
                            .description(GENERAL_ISSUE)
                            .build();

                        if let Err(err) = remove_components(&ctx, &component, Some(embed)).await {
                            let report = Report::new(err).wrap_err("failed to remove components");
                            warn!("{report:?}");
                        }

                        return Err(err);
                    }
                };

                let embed =
                    BGTagsEmbed::new(*included, *excluded, mapsets.len(), *effects, *difficulty)
                        .into_builder()
                        .build();

                if let Err(err) = remove_components(&ctx, &component, Some(embed)).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }

                if mapsets.is_empty() {
                    entry.remove();

                    return Ok(());
                }

                info!(
                    "Starting game with included: {} - excluded: {}",
                    included.join(','),
                    excluded.join(',')
                );

                let game =
                    GameWrapper::new(Arc::clone(&ctx), channel, mapsets, *effects, *difficulty)
                        .await;

                entry.insert(BgGameState::Running { game });
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(&ctx, &component, None).await {
                let report = Report::new(err).wrap_err("failed to remove components");
                warn!("{report:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_cancel(
    ctx: &Context,
    component: MessageComponentInteraction,
) -> BotResult<()> {
    let channel = component.channel_id;

    match ctx.bg_games().entry(channel) {
        Entry::Occupied(entry) => match entry.get() {
            BgGameState::Running { .. } => {
                if let Err(err) = remove_components(ctx, &component, None).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }

                return Ok(());
            }
            BgGameState::Setup { author, .. } => {
                if *author != component.user_id()? {
                    return Ok(());
                }

                let embed = EmbedBuilder::new()
                    .description("Aborted background game setup")
                    .build();

                entry.remove();
                remove_components(ctx, &component, Some(embed)).await?;
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(ctx, &component, None).await {
                let report = Report::new(err).wrap_err("failed to remove components");
                warn!("{report:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_effects(
    ctx: &Context,
    mut component: MessageComponentInteraction,
) -> BotResult<()> {
    match ctx.bg_games().entry(component.channel_id) {
        Entry::Occupied(mut entry) => match entry.get_mut() {
            BgGameState::Running { .. } => {
                if let Err(err) = remove_components(ctx, &component, None).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }
            }
            BgGameState::Setup {
                author, effects, ..
            } => {
                if *author != component.user_id()? {
                    return Ok(());
                }

                *effects = component
                    .data
                    .values
                    .iter()
                    .fold(Effects::empty(), |effects, value| {
                        effects
                            | match value.as_str() {
                                "blur" => Effects::Blur,
                                "contrast" => Effects::Contrast,
                                "flip_h" => Effects::FlipHorizontal,
                                "flip_v" => Effects::FlipVertical,
                                "grayscale" => Effects::Grayscale,
                                "invert" => Effects::Invert,
                                _ => {
                                    warn!("unknown effects `{value}`");

                                    return effects;
                                }
                            }
                    });

                let mut embed = component
                    .message
                    .embeds
                    .pop()
                    .ok_or(InvalidBgState::MissingEmbed)?;

                let field_opt = embed
                    .fields
                    .iter_mut()
                    .find(|field| field.name == "Effects");

                if let Some(field) = field_opt {
                    field.value = effects.join(", ");
                } else {
                    let field = EmbedField {
                        inline: false,
                        name: "Effects".to_owned(),
                        value: effects.join(", "),
                    };

                    embed.fields.push(field);
                }

                let data = InteractionResponseData {
                    embeds: Some(vec![embed]),
                    ..Default::default()
                };

                let response = InteractionResponse {
                    kind: InteractionResponseType::UpdateMessage,
                    data: Some(data),
                };

                let client = ctx.interaction();

                client
                    .create_response(component.id, &component.token, &response)
                    .exec()
                    .await?;
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(ctx, &component, None).await {
                let report = Report::new(err).wrap_err("failed to remove components");
                warn!("{report:?}");
            }
        }
    }

    Ok(())
}

async fn remove_components(
    ctx: &Context,
    component: &MessageComponentInteraction,
    embed: Option<Embed>,
) -> BotResult<()> {
    let data = InteractionResponseData {
        components: Some(Vec::new()),
        embeds: embed.map(|e| vec![e]),
        ..Default::default()
    };

    let response = InteractionResponse {
        kind: InteractionResponseType::UpdateMessage,
        data: Some(data),
    };

    let client = ctx.interaction();

    client
        .create_response(component.id, &component.token, &response)
        .exec()
        .await?;

    Ok(())
}

fn bg_components() -> Vec<Component> {
    let options = vec![
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Easy".to_owned(),
            value: "easy".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Hard".to_owned(),
            value: "hard".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Meme".to_owned(),
            value: "meme".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Weeb".to_owned(),
            value: "weeb".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "K-Pop".to_owned(),
            value: "kpop".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Farm".to_owned(),
            value: "farm".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Hard name".to_owned(),
            value: "hardname".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Alternate".to_owned(),
            value: "alt".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Blue sky".to_owned(),
            value: "bluesky".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "English".to_owned(),
            value: "english".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Streams".to_owned(),
            value: "streams".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Old".to_owned(),
            value: "old".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: None,
            emoji: None,
            label: "Tech".to_owned(),
            value: "tech".to_owned(),
        },
    ];

    let include_menu = SelectMenu {
        custom_id: "bg_start_include".to_owned(),
        disabled: false,
        max_values: Some(options.len() as u8),
        min_values: Some(0),
        options: options.clone(),
        placeholder: Some("Select which tags should be included".to_owned()),
    };

    let include_row = ActionRow {
        components: vec![Component::SelectMenu(include_menu)],
    };

    let exclude_menu = SelectMenu {
        custom_id: "bg_start_exclude".to_owned(),
        disabled: false,
        max_values: Some(options.len() as u8),
        min_values: Some(0),
        options,
        placeholder: Some("Select which tags should be excluded".to_owned()),
    };

    let exclude_row = ActionRow {
        components: vec![Component::SelectMenu(exclude_menu)],
    };

    let start_button = Button {
        custom_id: Some("bg_start_button".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Start".to_owned()),
        style: ButtonStyle::Success,
        url: None,
    };

    let cancel_button = Button {
        custom_id: Some("bg_start_cancel".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Cancel".to_owned()),
        style: ButtonStyle::Danger,
        url: None,
    };

    let button_row = ActionRow {
        components: vec![
            Component::Button(start_button),
            Component::Button(cancel_button),
        ],
    };

    let effects = vec![
        SelectMenuOption {
            default: false,
            description: Some("Blur the image".to_owned()),
            emoji: None,
            label: "blur".to_owned(),
            value: "blur".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: Some("Increase the color contrast".to_owned()),
            emoji: None,
            label: "Contrast".to_owned(),
            value: "contrast".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: Some("Flip the image horizontally".to_owned()),
            emoji: None,
            label: "Flip horizontal".to_owned(),
            value: "flip_h".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: Some("Flip the image vertically".to_owned()),
            emoji: None,
            label: "Flip vertical".to_owned(),
            value: "flip_v".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: Some("Grayscale the colors".to_owned()),
            emoji: None,
            label: "Grayscale".to_owned(),
            value: "grayscale".to_owned(),
        },
        SelectMenuOption {
            default: false,
            description: Some("Invert the colors".to_owned()),
            emoji: None,
            label: "Invert".to_owned(),
            value: "invert".to_owned(),
        },
    ];

    let effects_menu = SelectMenu {
        custom_id: "bg_start_effects".to_owned(),
        disabled: false,
        max_values: Some(effects.len() as u8),
        min_values: Some(0),
        options: effects,
        placeholder: Some("Modify images through effects".to_owned()),
    };

    let effects_row = ActionRow {
        components: vec![Component::SelectMenu(effects_menu)],
    };

    vec![
        Component::ActionRow(include_row),
        Component::ActionRow(exclude_row),
        Component::ActionRow(effects_row),
        Component::ActionRow(button_row),
    ]
}

pub async fn slash_bg(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    if let Some((_, BgGameState::Running { game })) = ctx.bg_games().remove(&command.channel_id) {
        if let Err(err) = game.stop() {
            let report = Report::new(err).wrap_err("failed to stop game");
            warn!("{report:?}");
        }
    }

    let mut mode = None;
    let mut difficulty = None;

    for option in &command.data.options {
        match option.value {
            CommandOptionValue::String(ref value) => match option.name.as_str() {
                "difficulty" => match value.as_str() {
                    "normal" => difficulty = Some(GameDifficulty::Normal),
                    "hard" => difficulty = Some(GameDifficulty::Hard),
                    "impossible" => difficulty = Some(GameDifficulty::Impossible),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                MODE => mode = parse_mode_option(value),
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    let difficulty = difficulty.unwrap_or_default();

    let state = match mode {
        Some(GameMode::STD) | None => {
            let components = bg_components();
            let author = command.user_id()?;

            let content = format!(
                "<@{author}> select which tags should be included \
                and which ones should be excluded, then start the game.\n\
                Only you can use the components below.",
            );

            let builder = MessageBuilder::new().embed(content).components(&components);
            command.create_message(&ctx, builder).await?;

            BgGameState::Setup {
                author,
                difficulty,
                effects: Effects::empty(),
                excluded: MapsetTags::empty(),
                included: MapsetTags::empty(),
            }
        }
        Some(GameMode::MNA) => {
            let mapsets = match ctx.psql().get_all_tags_mapset(GameMode::MNA).await {
                Ok(mapsets) => mapsets,
                Err(err) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            };

            let content = format!(
                "Starting mania background guessing game with {} different backgrounds",
                mapsets.len()
            );

            let builder = MessageBuilder::new().embed(content);
            command.create_message(&ctx, builder).await?;

            let game_fut = GameWrapper::new(
                Arc::clone(&ctx),
                command.channel_id,
                mapsets,
                Effects::empty(),
                difficulty,
            );

            BgGameState::Running {
                game: game_fut.await,
            }
        }
        Some(GameMode::TKO | GameMode::CTB) => unreachable!(),
    };

    ctx.bg_games().insert(command.channel_id, state);

    Ok(())
}

pub fn define_bg() -> MyCommand {
    let mode_choices = vec![
        CommandOptionChoice::String {
            name: OSU.to_owned(),
            value: OSU.to_owned(),
        },
        CommandOptionChoice::String {
            name: MANIA.to_owned(),
            value: MANIA.to_owned(),
        },
    ];

    let mode = MyCommandOption::builder(MODE, SPECIFY_MODE).string(mode_choices, false);

    let difficulty_choices = vec![
        CommandOptionChoice::String {
            name: "Normal".to_owned(),
            value: "normal".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Hard".to_owned(),
            value: "hard".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Impossible".to_owned(),
            value: "impossible".to_owned(),
        },
    ];

    let difficulty_description = "Increase difficulty by requiring better guessing";

    let difficulty_help = "Increase the difficulty.\n\
        The higher the difficulty, the more accurate guesses have to be in order to be accepted.";

    let difficulty = MyCommandOption::builder("difficulty", difficulty_description)
        .help(difficulty_help)
        .string(difficulty_choices, false);

    let description = "Start a new background guessing game";

    let help = "Start a new background guessing game.\n\
        Given part of a map's background, try to guess the **title** of the map's song.\n\
        You don't need to guess content in parentheses `(...)` or content after `ft.` or `feat.`.\n\n\
        Use these prefix commands to initiate with the game:\n\
        • `<bg s[kip]` / `<bg r[esolve]`: Resolve the current background and \
        give a new one with the same tag specs.\n\
        • `<bg h[int]`: Receive a hint (can be used multiple times).\n\
        • `<bg b[igger]`: Increase the radius of the displayed image (can be used multiple times).\n\
        • `<bg stop`: Resolve the current background and stop the game.
        • `<bg l[eaderboard] s[erver]`: Check out the global leaderboard for \
        amount of correct guesses. If `server` or `s` is added at the end, \
        I will only show members of this server.";

    MyCommand::new("bg", description)
        .help(help)
        .options(vec![mode, difficulty])
}

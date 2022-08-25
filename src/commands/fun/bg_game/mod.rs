use std::sync::Arc;

use command_macros::{command, SlashCommand};
use eyre::Report;
use rosu_v2::prelude::GameMode;
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::component::{
        button::ButtonStyle, select_menu::SelectMenuOption, ActionRow, Button, Component,
        SelectMenu,
    },
    channel::{thread::AutoArchiveDuration, ChannelType},
};

use crate::{
    commands::ThreadChannel,
    games::bg::{Effects, GameState, GameWrapper, MapsetTags},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, INVALID_ACTION_FOR_CHANNEL_TYPE, THREADS_UNAVAILABLE},
        interaction::InteractionCommand,
        Authored, ChannelExt, CowUtils, InteractionCommandExt,
    },
    BotResult, Context,
};

use self::{bigger::*, hint::*, rankings::*, skip::*, stop::*};

mod bigger;
mod hint;
mod rankings;
mod skip;
mod stop;
// mod tags; // TODO

#[command]
#[desc("Play the background guessing game, use `/bg` to start")]
#[alias("bg")]
#[flags(SKIP_DEFER)] // defer manually on specific subcommands
#[group(Games)]
pub async fn prefix_backgroundgame(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let mut args = args.map(|arg| arg.cow_to_ascii_lowercase());
    let arg = args.next();

    match arg.as_ref().map(|arg| arg.as_ref()) {
        None | Some("help") => {
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
            msg.create_message(&ctx, &builder).await?;

            Ok(())
        }
        Some("s" | "skip" | "r" | "resolve" | "start") => skip(ctx, msg).await,
        Some("h" | "hint") => hint(ctx, msg).await,
        Some("b" | "bigger" | "enhance") => bigger(ctx, msg).await,
        Some("stop" | "end" | "quit") => stop(ctx, msg).await,
        Some("l" | "lb" | "leaderboard") => {
            let arg = args.next();

            match arg.as_ref().map(|arg| arg.as_ref()) {
                Some("s" | "server") => leaderboard(ctx, msg, false).await,
                _ => leaderboard(ctx, msg, true).await,
            }
        }
        _ => {
            let prefix = ctx.guild_first_prefix(msg.guild_id).await;

            let content =
                format!("That's not a valid subcommand. Check `{prefix}bg` for more help.");

            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "bg",
    help = "Start a new background guessing game.\n\
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
    I will only show members of this server."
)]
#[flags(SKIP_DEFER)]
/// Start a new background guessing game
pub struct Bg {
    /// Specify a gamemode
    mode: Option<BgGameMode>,
    #[command(help = "Increase the difficulty.\n\
    The higher the difficulty, the more accurate guesses have to be in order to be accepted.")]
    /// Increase difficulty by requiring better guessing
    difficulty: Option<GameDifficulty>,
    /// Choose if a new thread should be started, defaults to staying in the channel
    thread: Option<ThreadChannel>,
}

#[derive(CommandOption, CreateOption)]
pub enum BgGameMode {
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "mania", value = "mania")]
    Mania,
}

#[derive(Copy, Clone, Debug, CommandOption, CreateOption)]
pub enum GameDifficulty {
    #[option(name = "Normal", value = "normal")]
    Normal,
    #[option(name = "Hard", value = "hard")]
    Hard,
    #[option(name = "Impossible", value = "impossible")]
    Impossible,
}

impl GameDifficulty {
    pub fn factor(self) -> f32 {
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

async fn slash_bg(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    let Bg {
        difficulty,
        mode,
        thread,
    } = Bg::from_interaction(command.input_data())?;
    let mut channel = command.channel_id;
    let author_user = command.user()?;
    let author = author_user.id;

    if let Some(ThreadChannel::Thread) = thread {
        if command.guild_id.is_none() {
            command.error(&ctx, THREADS_UNAVAILABLE).await?;

            return Ok(());
        }

        let kind = ChannelType::GuildPublicThread;
        let archive_dur = AutoArchiveDuration::Day;
        let thread_name = format!("Background guessing game of {}", author_user.name);

        let create_fut = ctx
            .http
            .create_thread(channel, &thread_name, kind)
            .unwrap()
            .auto_archive_duration(archive_dur)
            .exec();

        match create_fut.await {
            Ok(res) => channel = res.model().await?.id,
            Err(err) => {
                let content = match err.kind() {
                    ErrorType::Response {
                        error: ApiError::General(err),
                        ..
                    } => match err.code {
                        INVALID_ACTION_FOR_CHANNEL_TYPE => Some(THREADS_UNAVAILABLE),
                        _ => None,
                    },
                    _ => None,
                };

                match content {
                    Some(content) => {
                        command.error(&ctx, content).await?;

                        return Ok(());
                    }
                    None => {
                        let _ = command.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.into());
                    }
                }
            }
        }
    }

    if let Some(GameState::Running { game }) = ctx.bg_games().write(&channel).await.remove() {
        if let Err(err) = game.stop() {
            let report = Report::new(err).wrap_err("failed to stop game");
            warn!("{report:?}");
        }
    }

    let difficulty = difficulty.unwrap_or_default();

    let state = match mode {
        Some(BgGameMode::Osu) | None => {
            let components = bg_components();

            let content = format!(
                "<@{author}> select which tags should be included \
                and which ones should be excluded, then start the game.\n\
                Only you can use the components below.",
            );

            let builder = MessageBuilder::new().embed(content).components(components);

            if matches!(thread, Some(ThreadChannel::Thread)) {
                let res_builder = MessageBuilder::new().embed("Starting new thread...");
                command.callback(&ctx, res_builder, true).await?;

                channel.create_message(&ctx, &builder).await?;
            } else {
                command.callback(&ctx, builder, false).await?;
            }

            GameState::Setup {
                author,
                difficulty,
                effects: Effects::empty(),
                excluded: MapsetTags::empty(),
                included: MapsetTags::empty(),
            }
        }
        Some(BgGameMode::Mania) => {
            let mapsets = match ctx.psql().get_all_tags_mapset(GameMode::Mania).await {
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

            if matches!(thread, Some(ThreadChannel::Thread)) {
                let res_builder = MessageBuilder::new().embed("Starting new thread...");
                command.callback(&ctx, res_builder, true).await?;

                channel.create_message(&ctx, &builder).await?;
            } else {
                command.callback(&ctx, builder, false).await?;
            }

            let game_fut = GameWrapper::new(
                Arc::clone(&ctx),
                channel,
                mapsets,
                Effects::empty(),
                difficulty,
            );

            GameState::Running {
                game: game_fut.await,
            }
        }
    };

    ctx.bg_games().own(channel).await.insert(state);

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

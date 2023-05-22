use std::sync::Arc;

use bathbot_macros::{command, SlashCommand};
use bathbot_model::Effects;
use bathbot_psql::model::games::DbMapTagsParams;
use bathbot_util::{
    constants::{GENERAL_ISSUE, INVALID_ACTION_FOR_CHANNEL_TYPE, THREADS_UNAVAILABLE},
    CowUtils, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::GameMode;
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    channel::{thread::AutoArchiveDuration, ChannelType},
    guild::Permissions,
};

use self::{bigger::*, hint::*, rankings::*, skip::*, stop::*};
use crate::{
    active::{
        impls::{BackgroundGame, BackgroundGameSetup},
        ActiveMessages,
    },
    commands::ThreadChannel,
    util::{interaction::InteractionCommand, Authored, ChannelExt, InteractionCommandExt},
    Context,
};

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
    permissions: Option<Permissions>,
) -> Result<()> {
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
            msg.create_message(&ctx, &builder, permissions).await?;

            Ok(())
        }
        Some("s" | "skip" | "r" | "resolve" | "start") => skip(ctx, msg).await,
        Some("h" | "hint") => hint(ctx, msg, permissions).await,
        Some("b" | "bigger" | "enhance") => bigger(ctx, msg, permissions).await,
        Some("stop" | "end" | "quit") => stop(ctx, msg).await,
        Some("l" | "lb" | "leaderboard") => {
            let arg = args.next();

            match arg.as_ref().map(|arg| arg.as_ref()) {
                Some("s" | "server") => leaderboard(ctx, msg, false).await,
                _ => leaderboard(ctx, msg, true).await,
            }
        }
        _ => {
            let prefix = ctx.guild_config().first_prefix(msg.guild_id).await;

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
    desc = "Start a new background guessing game",
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
pub struct Bg {
    #[command(desc = "Specify a gamemode")]
    mode: Option<BgGameMode>,
    #[command(
        desc = "Increase difficulty by requiring better guessing",
        help = "Increase the difficulty.\n\
        The higher the difficulty, the more accurate guesses have to be in order to be accepted."
    )]
    difficulty: Option<GameDifficulty>,
    #[command(
        desc = "Choose if a new thread should be started, defaults to staying in the channel"
    )]
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

async fn slash_bg(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let Bg {
        difficulty,
        mode,
        thread,
    } = Bg::from_interaction(command.input_data())?;

    let can_view_channel = command.permissions.map_or(true, |permissions| {
        permissions.contains(Permissions::VIEW_CHANNEL)
    });

    if !can_view_channel {
        let content = r#"I'm lacking the "View Channel" permission in this channel"#;
        command.error_callback(&ctx, content).await?;

        return Ok(());
    }

    let can_attach_files = command.permissions.map_or(true, |permissions| {
        permissions.contains(Permissions::ATTACH_FILES)
    });

    if !can_attach_files {
        let content = "I'm lacking the permission to attach files";
        command.error_callback(&ctx, content).await?;

        return Ok(());
    }

    let mut channel = command.channel_id;
    let author_user = command.user()?;
    let author = author_user.id;

    if let Some(ThreadChannel::Thread) = thread {
        if command.guild_id.is_none() {
            command.error_callback(&ctx, THREADS_UNAVAILABLE).await?;

            return Ok(());
        }

        let can_create_thread = command.permissions.map_or(true, |permissions| {
            permissions.contains(Permissions::CREATE_PUBLIC_THREADS)
        });

        if !can_create_thread {
            let content = r#"I'm lacking "Create Public Threads" permission in this channel"#;
            command.error_callback(&ctx, content).await?;

            return Ok(());
        }

        let can_send_msgs = command.permissions.map_or(true, |permissions| {
            permissions.contains(Permissions::SEND_MESSAGES_IN_THREADS)
        });

        if !can_send_msgs {
            let content =
                r#"I'm lacking the "Send Messages in Threads" permission in this channel"#;
            command.error_callback(&ctx, content).await?;

            return Ok(());
        }

        let kind = ChannelType::PublicThread;
        let archive_dur = AutoArchiveDuration::Day;
        let thread_name = format!("Background guessing game of {}", author_user.name);

        let create_fut = ctx
            .http
            .create_thread(channel, &thread_name, kind)
            .unwrap()
            .auto_archive_duration(archive_dur);

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
                        command.error_callback(&ctx, content).await?;

                        return Ok(());
                    }
                    None => {
                        let _ = command.error_callback(&ctx, GENERAL_ISSUE).await;

                        return Err(Report::new(err).wrap_err("failed to create thread"));
                    }
                }
            }
        }
    } else {
        let can_send_msgs = command.permissions.map_or(true, |permissions| {
            permissions.contains(Permissions::SEND_MESSAGES)
        });

        if !can_send_msgs {
            let content = r#"I'm lacking the "Send Messages" permission in this channel"#;
            command.error_callback(&ctx, content).await?;

            return Ok(());
        }
    }

    if let Some(game) = ctx.bg_games().write(&channel).await.remove() {
        if let Err(err) = game.stop() {
            warn!(?err, "Failed to stop game");
        }
    }

    let difficulty = difficulty.unwrap_or_default();

    match mode {
        Some(BgGameMode::Osu) | None => {
            let setup = BackgroundGameSetup::new(difficulty, author);

            if matches!(thread, Some(ThreadChannel::Thread)) {
                let res_builder = MessageBuilder::new().embed("Starting new thread...");
                command.callback(&ctx, res_builder, true).await?;

                ActiveMessages::builder(setup).begin(ctx, channel).await
            } else {
                ActiveMessages::builder(setup)
                    .begin(ctx, &mut command)
                    .await
            }
        }
        Some(BgGameMode::Mania) => {
            let params = DbMapTagsParams::new(GameMode::Mania);

            let entries = match ctx.games().bggame_tags(params).await {
                Ok(entries) => entries,
                Err(err) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("failed to get all tagged mania mapsets"));
                }
            };

            let content = format!(
                "Starting mania background guessing game with {} different backgrounds",
                entries.tags.len()
            );

            let builder = MessageBuilder::new().embed(content);

            if matches!(thread, Some(ThreadChannel::Thread)) {
                let res_builder = MessageBuilder::new().embed("Starting new thread...");
                command.callback(&ctx, res_builder, true).await?;

                channel.create_message(&ctx, &builder, None).await?;
            } else {
                command.callback(&ctx, builder, false).await?;
            }

            let game_fut = BackgroundGame::new(
                Arc::clone(&ctx),
                channel,
                entries,
                Effects::empty(),
                difficulty,
            );

            ctx.bg_games().own(channel).await.insert(game_fut.await);

            Ok(())
        }
    }
}

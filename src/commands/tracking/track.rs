use std::sync::Arc;

use command_macros::command;
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuError};
use time::OffsetDateTime;

use crate::{
    core::commands::CommandOrigin,
    embeds::{EmbedData, TrackEmbed},
    util::{builder::MessageBuilder, constants::OSU_API_ISSUE, ChannelExt},
    Context,
};

use super::TrackArgs;

pub(super) async fn track(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: TrackArgs,
) -> Result<()> {
    let TrackArgs {
        name,
        mode,
        limit,
        mut more_names,
    } = args;

    more_names.push(name);

    if let Some(name) = more_names.iter().find(|name| name.len() > 15) {
        let content = format!("`{name}` is too long for an osu! username");

        return orig.error(&ctx, content).await;
    }

    let limit = match limit {
        Some(limit) if limit == 0 || limit > 100 => {
            let content = "The given limit must be between 1 and 100";

            return orig.error(&ctx, content).await;
        }
        Some(limit) => limit as usize,
        None => {
            let guild = orig.guild_id().unwrap();

            ctx.guild_track_limit(guild).await as usize
        }
    };

    let mode = mode.unwrap_or(GameMode::Osu);

    let users = match super::get_names(&ctx, &more_names, mode).await {
        Ok(map) => map,
        Err((OsuError::NotFound, name)) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err((err, _)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get names");

            return Err(report);
        }
    };

    let channel = orig.channel_id();
    let mut success = Vec::with_capacity(users.len());
    let mut failure = Vec::new();

    for (username, user_id) in users {
        let add_fut = ctx.tracking().add(
            user_id,
            mode,
            OffsetDateTime::now_utc(),
            channel,
            limit,
            ctx.psql(),
        );

        match add_fut.await {
            Ok(true) => success.push(username),
            Ok(false) => failure.push(username),
            Err(err) => {
                warn!("{:?}", err.wrap_err("Failed to add tracked entry"));

                let embed = TrackEmbed::new(mode, success, failure, Some(username), limit).build();

                let builder = MessageBuilder::new().embed(embed);
                orig.create_message(&ctx, &builder).await?;

                return Ok(());
            }
        }
    }

    let embed = TrackEmbed::new(mode, success, failure, None, limit);
    let builder = MessageBuilder::new().embed(embed.build());
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

#[command]
#[desc("Track osu!standard user top scores")]
#[help(
    "Track osu!standard user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `track42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `track limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[limit=number] [username1] [username2] ...")]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Tracking)]
async fn prefix_track(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Osu), args).await {
        Ok(args) => track(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Track mania user top scores")]
#[help(
    "Track mania user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `trackmania42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `trackmania limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[limit=number] [username1] [username2] ...")]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Tracking)]
pub async fn prefix_trackmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Mania), args).await {
        Ok(args) => track(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Track taiko user top scores")]
#[help(
    "Track taiko user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `tracktaiko42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `tracktaiko limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[limit=number] [username1] [username2] ...")]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Tracking)]
pub async fn prefix_tracktaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Taiko), args).await {
        Ok(args) => track(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Track ctb user top scores")]
#[help(
    "Track ctb user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `trackctb42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `trackctb limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[limit=number] [username1] [username2] ...")]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[alias("trackingcatch")]
#[group(Tracking)]
pub async fn prefix_trackctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Catch), args).await {
        Ok(args) => track(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

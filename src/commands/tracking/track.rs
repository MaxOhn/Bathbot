use super::TrackArgs;
use crate::{
    embeds::{EmbedData, TrackEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use chrono::Utc;
use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use hashbrown::HashSet;
use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;

pub(super) async fn _track(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: TrackArgs,
) -> BotResult<()> {
    let mut names: HashSet<_> = args.more_names.into_iter().collect();
    names.insert(args.name);

    if let Some(name) = names.iter().find(|name| name.len() > 15) {
        let content = format!("`{}` is too long for an osu! username", name);

        return data.error(&ctx, content).await;
    }

    let limit = match args.limit {
        Some(limit) if limit == 0 || limit > 100 => {
            let content = "The given limit must be between 1 and 100";

            return data.error(&ctx, content).await;
        }
        Some(limit) => limit,
        None => 50,
    };

    let count = names.len();
    let mode = args.mode.unwrap_or(GameMode::STD);

    // Retrieve all users
    let mut user_futs: FuturesUnordered<_> = names
        .into_iter()
        .map(|name| {
            ctx.osu()
                .user(name.as_str())
                .mode(mode)
                .map(move |result| (name, result))
        })
        .collect();

    let mut users = Vec::with_capacity(count);

    while let Some((name, result)) = user_futs.next().await {
        match result {
            Ok(user) => users.push((user.user_id, user.username)),
            Err(OsuError::NotFound) => {
                let content = format!("User `{}` was not found", name);

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    // Free &ctx again
    drop(user_futs);

    let channel = data.channel_id();
    let mut success = Vec::with_capacity(users.len());
    let mut failure = Vec::new();

    for (user_id, username) in users {
        let add_fut = ctx
            .tracking()
            .add(user_id, mode, Utc::now(), channel, limit, ctx.psql());

        match add_fut.await {
            Ok(true) => success.push(username),
            Ok(false) => failure.push(username),
            Err(why) => {
                unwind_error!(warn, why, "Error while adding tracked entry: {}");

                let embed = TrackEmbed::new(mode, success, failure, Some(username), limit)
                    .into_builder()
                    .build();
                let builder = MessageBuilder::new().embed(embed);
                data.create_message(&ctx, builder).await?;

                return Ok(());
            }
        }
    }

    let embed = TrackEmbed::new(mode, success, failure, None, limit)
        .into_builder()
        .build();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

#[command]
#[authority()]
#[short_desc("Track osu!standard user(s') top scores")]
#[long_desc(
    "Track osu!standard user(s') top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `track42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `-limit` \
    followed by a number, e.g. `track -limit 42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
pub async fn track(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let track_args = match TrackArgs::args(&ctx, &mut args, num, Some(GameMode::STD)) {
                Ok(args) => args,
                Err(content) => return msg.error(&ctx, content).await,
            };

            _track(ctx, CommandData::Message { msg, args, num }, track_args).await
        }
        CommandData::Interaction { command } => super::slash_track(ctx, command).await,
    }
}

#[command]
#[authority()]
#[short_desc("Track mania user(s') top scores")]
#[long_desc(
    "Track mania user(s') top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `trackmania42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `-limit` \
    followed by a number, e.g. `trackmania -limit 42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
pub async fn trackmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let track_args = match TrackArgs::args(&ctx, &mut args, num, Some(GameMode::MNA)) {
                Ok(args) => args,
                Err(content) => return msg.error(&ctx, content).await,
            };

            _track(ctx, CommandData::Message { msg, args, num }, track_args).await
        }
        CommandData::Interaction { command } => super::slash_track(ctx, command).await,
    }
}

#[command]
#[authority()]
#[short_desc("Track taiko user(s') top scores")]
#[long_desc(
    "Track taiko user(s') top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `tracktaiko42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `-limit` \
    followed by a number, e.g. `tracktaiko -limit 42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
pub async fn tracktaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let track_args = match TrackArgs::args(&ctx, &mut args, num, Some(GameMode::TKO)) {
                Ok(args) => args,
                Err(content) => return msg.error(&ctx, content).await,
            };

            _track(ctx, CommandData::Message { msg, args, num }, track_args).await
        }
        CommandData::Interaction { command } => super::slash_track(ctx, command).await,
    }
}

#[command]
#[authority()]
#[short_desc("Track ctb user(s') top scores")]
#[long_desc(
    "Track ctb user(s') top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `trackctb42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `-limit` \
    followed by a number, e.g. `trackctb -limit 42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
pub async fn trackctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let track_args = match TrackArgs::args(&ctx, &mut args, num, Some(GameMode::CTB)) {
                Ok(args) => args,
                Err(content) => return msg.error(&ctx, content).await,
            };

            _track(ctx, CommandData::Message { msg, args, num }, track_args).await
        }
        CommandData::Interaction { command } => super::slash_track(ctx, command).await,
    }
}

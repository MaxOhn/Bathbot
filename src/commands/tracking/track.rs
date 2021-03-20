use crate::{
    arguments::{Args, MultNameLimitArgs},
    embeds::{EmbedData, TrackEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use chrono::Utc;
use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use rosu_v2::prelude::{GameMode, OsuError};
use std::{collections::HashSet, sync::Arc};
use twilight_model::channel::Message;

async fn track_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    num: Option<usize>,
) -> BotResult<()> {
    let args = match MultNameLimitArgs::new(&ctx, args, 10) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    let names = if args.names.is_empty() {
        let content = "You need to specify at least one osu username";

        return msg.error(&ctx, content).await;
    } else {
        args.names.into_iter().collect::<HashSet<_>>()
    };

    if let Some(name) = names.iter().find(|name| name.len() > 15) {
        let content = format!("`{}` is too long for an osu! username", name);

        return msg.error(&ctx, content).await;
    }

    let limit = match args.limit.or(num) {
        Some(limit) if limit == 0 || limit > 100 => {
            let content = "The given limit must be between 1 and 100";

            return msg.error(&ctx, content).await;
        }
        Some(limit) => limit,
        None => 50,
    };

    let count = names.len();

    // Retrieve all users
    let mut user_futs = names
        .into_iter()
        .map(|name| {
            ctx.osu()
                .user(name.as_str())
                .mode(mode)
                .map(move |result| (name, result))
        })
        .collect::<FuturesUnordered<_>>();

    let mut users = Vec::with_capacity(count);

    while let Some((name, result)) = user_futs.next().await {
        match result {
            Ok(user) => users.push((user.user_id, user.username)),
            Err(OsuError::NotFound) => {
                let content = format!("User `{}` was not found", name);

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    // Free &ctx again
    drop(user_futs);

    if users.is_empty() {
        let content = "None of the given users were found by the API";

        return msg.error(&ctx, content).await;
    }

    let channel = msg.channel_id;
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
                    .build_owned()
                    .build()?;

                return msg.build_response(&ctx, |m| m.embed(embed)).await;
            }
        }
    }
    let embed = TrackEmbed::new(mode, success, failure, None, limit)
        .build_owned()
        .build()?;

    msg.build_response(&ctx, |m| m.embed(embed)).await?;

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
pub async fn track(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    track_main(GameMode::STD, ctx, msg, args, num).await
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
pub async fn trackmania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    track_main(GameMode::MNA, ctx, msg, args, num).await
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
pub async fn tracktaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    track_main(GameMode::TKO, ctx, msg, args, num).await
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
pub async fn trackctb(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
    num: Option<usize>,
) -> BotResult<()> {
    track_main(GameMode::CTB, ctx, msg, args, num).await
}

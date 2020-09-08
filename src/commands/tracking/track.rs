use crate::{
    arguments::{Args, MultNameLimitArgs},
    embeds::{EmbedData, TrackEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use chrono::Utc;
use futures::future::{try_join_all, TryFutureExt};
use rosu::{backend::UserRequest, models::GameMode};
use std::{collections::HashSet, sync::Arc};
use twilight::model::channel::Message;

async fn track_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    // let guild_id = msg.guild_id.unwrap().0;
    // if guild_id != 277469642908237826 && guild_id != 297072529426612224 {
    //     let content = "Top score tracking is currently in its testing phase, hence unavailable.";
    //     return msg.error(&ctx, content).await;
    // }
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

    let limit = match args.limit {
        Some(limit) if limit == 0 || limit > 100 => {
            let content = "The given limit must be between 1 and 100";
            return msg.error(&ctx, content).await;
        }
        Some(limit) => limit,
        None => 100,
    };

    // Retrieve all users
    let user_futs = names.into_iter().map(|name| {
        UserRequest::with_username(&name)
            .unwrap_or_else(|why| panic!("Invalid username `{}`: {}", name, why))
            .mode(mode)
            .queue_single(ctx.osu())
            .map_ok(move |user| (name, user))
    });
    let users: Vec<(u32, String)> = match try_join_all(user_futs).await {
        Ok(users) => match users.iter().find(|(_, user)| user.is_none()) {
            Some((name, _)) => {
                let content = format!("User `{}` was not found", name);
                return msg.error(&ctx, content).await;
            }
            None => users
                .into_iter()
                .filter_map(|(name, user)| user.map(|user| (user.user_id, name)))
                .collect(),
        },
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    if users.is_empty() {
        let content = "None of the given users were found by the API";
        return msg.error(&ctx, content).await;
    }
    let channel = msg.channel_id;
    let mut success = Vec::with_capacity(users.len());
    let mut failure = Vec::new();
    for (user_id, username) in users {
        match ctx
            .tracking()
            .add(user_id, mode, Utc::now(), channel, limit, ctx.psql())
            .await
        {
            Ok(true) => success.push(username),
            Ok(false) => failure.push(username),
            Err(why) => {
                warn!("Error while adding tracked entry: {}", why);
                let embed = TrackEmbed::new(mode, success, failure, Some(username), limit)
                    .build()
                    .build()?;
                return msg.build_response(&ctx, |m| m.embed(embed)).await;
            }
        }
    }
    let embed = TrackEmbed::new(mode, success, failure, None, limit)
        .build()
        .build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[authority()]
#[short_desc("Track osu!standard user(s)' top scores")]
#[long_desc(
    "Track osu! standarf user(s)' top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation.\n\
    To provide a limit, specify `-limit` followed by a number \
    between 1 and 100, defaults to 100."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
pub async fn track(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[authority()]
#[short_desc("Track mania user(s)' top scores")]
#[long_desc(
    "Track mania user(s)' top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation.\n\
    To provide a limit, specify `-limit` followed by a number \
    between 1 and 100, defaults to 100."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[aliases("tm")]
pub async fn trackmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[authority()]
#[short_desc("Track taiko user(s)' top scores")]
#[long_desc(
    "Track taiko user(s)' top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation.\n\
    To provide a limit, specify `-limit` followed by a number \
    between 1 and 100, defaults to 100."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[aliases("tt")]
pub async fn tracktaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[authority()]
#[short_desc("Track ctb user(s)' top scores")]
#[long_desc(
    "Track ctb user(s)' top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation.\n\
    To provide a limit, specify `-limit` followed by a number \
    between 1 and 100, defaults to 100."
)]
#[usage("[-limit number] [username1] [username2] ...")]
#[example(
    "badewanne3 \"freddie benson\" peppy -limit 23",
    "-limit 45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[aliases("tc")]
pub async fn trackctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::CTB, ctx, msg, args).await
}

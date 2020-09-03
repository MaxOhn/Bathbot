use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, TrackEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use chrono::{DateTime, Utc};
use futures::future::{try_join_all, TryFutureExt};
use rosu::{backend::BestRequest, models::GameMode};
use std::{collections::HashSet, sync::Arc};
use twilight::model::channel::Message;

async fn track_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = MultNameArgs::new(&ctx, args, 10);
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

    // Retrieve all users' top scores
    let req_futs = names.into_iter().map(|name| {
        BestRequest::with_username(&name)
            .unwrap_or_else(|why| panic!("Invalid username `{}`: {}", name, why))
            .mode(mode)
            .queue(ctx.osu())
            .map_ok(move |scores| (name, scores))
    });
    let last_dates: Vec<(u32, String, DateTime<Utc>)> = match try_join_all(req_futs).await {
        Ok(all_scores) => all_scores
            .into_iter()
            .filter_map(|(name, scores)| match scores.first() {
                Some(score) => {
                    let user_id = score.user_id;
                    let last_date = scores.into_iter().map(|score| score.date).max().unwrap();
                    Some((user_id, name, last_date))
                }
                None => None,
            })
            .collect(),
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    if last_dates.is_empty() {
        let content = "None of the given users have any top scores";
        return msg.error(&ctx, content).await;
    }
    let channel = msg.channel_id;
    let mut success = Vec::with_capacity(last_dates.len());
    let mut failure = Vec::new();
    for (user_id, username, last_top_score) in last_dates {
        match ctx
            .tracking()
            .write()
            .await
            .add(user_id, mode, channel, last_top_score, ctx.psql())
            .await
        {
            Ok(true) => success.push(username),
            Ok(false) => failure.push(username),
            Err(why) => {
                warn!("Error while adding tracked entry: {}", why);
                let embed = TrackEmbed::new(mode, success, failure, Some(username))
                    .build()
                    .build()?;
                return msg.build_response(&ctx, |m| m.embed(embed)).await;
            }
        }
    }
    let embed = TrackEmbed::new(mode, success, failure, None)
        .build()
        .build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[short_desc("Track a user's top scores")]
#[long_desc(
    "Track a user's top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
pub async fn track(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Track a mania user's top scores")]
#[long_desc(
    "Track a mania user's top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
#[aliases("tm")]
pub async fn trackmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Track a taiko user's top scores")]
#[long_desc(
    "Track a taiko user's top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
#[aliases("tt")]
pub async fn tracktaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Track a ctb user's top scores")]
#[long_desc(
    "Track a ctb user's top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
#[aliases("tc")]
pub async fn trackctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    track_main(GameMode::CTB, ctx, msg, args).await
}

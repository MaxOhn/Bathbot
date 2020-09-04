use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, UntrackEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use futures::future::{try_join_all, TryFutureExt};
use rayon::prelude::*;
use rosu::{
    backend::UserRequest,
    models::{GameMode, User},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use twilight::model::channel::Message;

async fn untrack_main(
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

    // Retrieve all users
    let user_futs = names.into_iter().map(|name| {
        UserRequest::with_username(&name)
            .unwrap_or_else(|why| panic!("Invalid username `{}`: {}", name, why))
            .mode(mode)
            .queue_single(ctx.osu())
            .map_ok(move |user| (name, user))
    });
    let users: HashMap<String, User> = match try_join_all(user_futs).await {
        Ok(users) => match users.par_iter().find_any(|(_, user)| user.is_none()) {
            Some((name, _)) => {
                let content = format!("User `{}` was not found", name);
                return msg.error(&ctx, content).await;
            }
            None => users
                .into_iter()
                .filter_map(|(name, user)| user.map(|user| (name, user)))
                .collect(),
        },
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let channel = msg.channel_id;
    let mut success = Vec::with_capacity(users.len());
    let mut failure = Vec::new();
    {
        for (name, User { user_id, .. }) in users {
            match ctx
                .tracking()
                .remove(user_id, mode, channel, ctx.psql())
                .await
            {
                Ok(true) => success.push(name),
                Ok(false) => failure.push(name),
                Err(why) => {
                    warn!("Error while adding tracked entry: {}", why);
                    let embed = UntrackEmbed::new(mode, success, failure, Some(name))
                        .build()
                        .build()?;
                    return msg.build_response(&ctx, |m| m.embed(embed)).await;
                }
            }
        }
    }
    let embed = UntrackEmbed::new(mode, success, failure, None)
        .build()
        .build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[authority()]
#[short_desc("Untrack user(s) in a channel")]
#[long_desc(
    "Stop notifying a channel about new plays in a user's top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
pub async fn untrack(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    untrack_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[authority()]
#[short_desc("Untrack a maniauser in a channel")]
#[long_desc(
    "Stop notifying a channel about new plays in a user's top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
#[aliases("utm")]
pub async fn untrackmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    untrack_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[authority()]
#[short_desc("Untrack a taiko user in a channel")]
#[long_desc(
    "Stop notifying a channel about new plays in a user's top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
#[aliases("utt")]
pub async fn untracktaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    untrack_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[authority()]
#[short_desc("Untrack a ctb user in a channel")]
#[long_desc(
    "Stop notifying a channel about new plays in a user's top100.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
#[aliases("utc")]
pub async fn untrackctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    untrack_main(GameMode::CTB, ctx, msg, args).await
}

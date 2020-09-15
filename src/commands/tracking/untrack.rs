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
use twilight_model::channel::Message;

#[command]
#[authority()]
#[short_desc("Untrack user(s) in a channel")]
#[long_desc(
    "Stop notifying a channel about new plays in a user's top100.\n\
    Specified users will be untracked for all modes.\n\
    You can specify up to ten usernames per command invokation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
async fn untrack(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let mode = GameMode::STD;
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
    let mut success = HashSet::with_capacity(users.len());
    for (name, User { user_id, .. }) in users.iter() {
        match ctx
            .tracking()
            .remove_user(*user_id, channel, ctx.psql())
            .await
        {
            Ok(_) => success.insert(name),
            Err(why) => {
                warn!("Error while adding tracked entry: {}", why);
                return send_message(&ctx, msg, Some(name), success).await;
            }
        };
    }
    send_message(&ctx, msg, None, success).await?;
    Ok(())
}

async fn send_message(
    ctx: &Context,
    msg: &Message,
    name: Option<&String>,
    success: HashSet<&String>,
) -> BotResult<()> {
    let success = success.into_iter().collect();
    let embed = UntrackEmbed::new(success, name).build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await
}

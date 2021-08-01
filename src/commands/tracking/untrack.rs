use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, UntrackEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use futures::stream::{FuturesUnordered, StreamExt};
use rosu_v2::prelude::{GameMode, OsuError};
use std::{collections::HashSet, sync::Arc};
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

    let channel = msg.channel_id;
    let mut success = HashSet::with_capacity(users.len());

    for (user_id, username) in users.into_iter() {
        match ctx
            .tracking()
            .remove_user(user_id, channel, ctx.psql())
            .await
        {
            Ok(_) => success.insert(username),
            Err(why) => {
                warn!("Error while adding tracked entry: {}", why);

                return send_message(&ctx, msg, Some(&username), success).await;
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
    success: HashSet<String>,
) -> BotResult<()> {
    let success = success.into_iter().collect();
    let embed = &[UntrackEmbed::new(success, name).into_builder().build()];

    msg.build_response(&ctx, |m| m.embeds(embed)).await
}

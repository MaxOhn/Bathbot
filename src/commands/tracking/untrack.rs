use super::TrackArgs;
use crate::{
    embeds::{EmbedData, UntrackEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use hashbrown::HashSet;
use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;

#[command]
#[authority()]
#[short_desc("Untrack user(s) in a channel")]
#[long_desc(
    "Stop notifying a channel about new plays in a user's top100.\n\
    Specified users will be untracked for all modes.\n\
    You can specify up to ten usernames per command invocation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
async fn untrack(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let track_args = match TrackArgs::args(&ctx, &mut args, num, None) {
                Ok(args) => args,
                Err(content) => return msg.error(&ctx, content).await,
            };

            _untrack(ctx, CommandData::Message { msg, args, num }, track_args).await
        }
        CommandData::Interaction { command } => super::slash_track(ctx, command).await,
    }
}

pub(super) async fn _untrack(ctx: Arc<Context>, data: CommandData<'_>, args: TrackArgs) -> BotResult<()> {
    let mode = args.mode.unwrap_or(GameMode::STD);

    let mut names: HashSet<_> = args.more_names.into_iter().collect();
    names.insert(args.name);

    if let Some(name) = names.iter().find(|name| name.len() > 15) {
        let content = format!("`{}` is too long for an osu! username", name);

        return data.error(&ctx, content).await;
    }

    let count = names.len();

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

                return send_message(&ctx, data, Some(&username), success).await;
            }
        };
    }

    send_message(&ctx, data, None, success).await?;

    Ok(())
}

async fn send_message(
    ctx: &Context,
    data: CommandData<'_>,
    name: Option<&String>,
    success: HashSet<String>,
) -> BotResult<()> {
    let success = success.into_iter().collect();
    let embed = UntrackEmbed::new(success, name).into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(ctx, builder).await?;

    Ok(())
}

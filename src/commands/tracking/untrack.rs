use std::sync::Arc;

use command_macros::command;
use hashbrown::HashSet;
use rosu_v2::prelude::{GameMode, OsuError, Username};

use crate::{
    core::commands::CommandOrigin,
    embeds::{EmbedData, UntrackEmbed},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    },
    BotResult, Context,
};

use super::TrackArgs;

#[command]
#[desc("Untrack user top scores in a channel")]
#[help(
    "Stop notifying a channel about new plays in a user's top100.\n\
    Specified users will be untracked for all modes.\n\
    You can specify up to ten usernames per command invocation."
)]
#[usage("[username1] [username2] ...")]
#[example("badewanne3 cookiezi \"freddie benson\" peppy")]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Tracking)]
async fn prefix_untrack(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TrackArgs::args(&ctx, &mut args, None).await {
        Ok(Ok(args)) => untrack(ctx, msg.into(), args).await,
        Ok(Err(content)) => return msg.error(&ctx, content).await,
        Err(err) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    }
}

pub async fn untrack(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: TrackArgs) -> BotResult<()> {
    let TrackArgs {
        name,
        mode,
        mut more_names,
        ..
    } = args;

    more_names.push(name);

    if let Some(name) = more_names.iter().find(|name| name.len() > 15) {
        let content = format!("`{name}` is too long for an osu! username");

        return orig.error(&ctx, content).await;
    }

    let mode = mode.unwrap_or(GameMode::STD);

    let users = match super::get_names(&ctx, &more_names, mode).await {
        Ok(map) => map,
        Err((OsuError::NotFound, name)) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err((err, _)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let channel = orig.channel_id();
    let mut success = HashSet::with_capacity(users.len());

    for (username, user_id) in users.into_iter() {
        let remove_fut = ctx
            .tracking()
            .remove_user(user_id, Some(mode), channel, ctx.psql());

        match remove_fut.await {
            Ok(_) => success.insert(username),
            Err(err) => {
                warn!("Error while adding tracked entry: {err}");

                return send_message(&ctx, orig, Some(&username), success).await;
            }
        };
    }

    send_message(&ctx, orig, None, success).await?;

    Ok(())
}

async fn send_message(
    ctx: &Context,
    orig: CommandOrigin<'_>,
    name: Option<&Username>,
    success: HashSet<Username>,
) -> BotResult<()> {
    let success = success.into_iter().collect();
    let embed = UntrackEmbed::new(success, name).into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(ctx, &builder).await?;

    Ok(())
}

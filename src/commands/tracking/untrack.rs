use std::sync::Arc;

use hashbrown::HashSet;
use rosu_v2::prelude::{GameMode, OsuError, Username};

use crate::{
    embeds::{EmbedData, UntrackEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use super::TrackArgs;

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
            let track_args = match TrackArgs::args(&ctx, &mut args, num, None).await {
                Ok(Ok(args)) => args,
                Ok(Err(content)) => return msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    return Err(why);
                }
            };

            _untrack(ctx, CommandData::Message { msg, args, num }, track_args).await
        }
        CommandData::Interaction { command } => super::slash_track(ctx, *command).await,
    }
}

pub(super) async fn _untrack(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: TrackArgs,
) -> BotResult<()> {
    let TrackArgs {
        name,
        mode,
        mut more_names,
        ..
    } = args;

    more_names.push(name);

    if let Some(name) = more_names.iter().find(|name| name.len() > 15) {
        let content = format!("`{name}` is too long for an osu! username");

        return data.error(&ctx, content).await;
    }

    let mode = mode.unwrap_or(GameMode::STD);

    let users = match super::get_names(&ctx, &more_names, mode).await {
        Ok(map) => map,
        Err((OsuError::NotFound, name)) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err((err, _)) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let channel = data.channel_id();
    let mut success = HashSet::with_capacity(users.len());

    for (username, user_id) in users.into_iter() {
        let remove_fut = ctx
            .tracking()
            .remove_user(user_id, Some(mode), channel, ctx.psql());

        match remove_fut.await {
            Ok(_) => success.insert(username),
            Err(err) => {
                warn!("Error while adding tracked entry: {err}");

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
    name: Option<&Username>,
    success: HashSet<Username>,
) -> BotResult<()> {
    let success = success.into_iter().collect();
    let embed = UntrackEmbed::new(success, name).into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(ctx, builder).await?;

    Ok(())
}

use bathbot_macros::command;
use bathbot_util::{constants::OSU_API_ISSUE, MessageBuilder};
use eyre::{Report, Result};
use hashbrown::HashSet;
use rosu_v2::prelude::{GameMode, OsuError, Username};

use super::TrackArgs;
use crate::{
    core::commands::CommandOrigin,
    embeds::{EmbedData, UntrackEmbed},
    util::ChannelExt,
    Context,
};

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
async fn prefix_untrack(msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(None, args).await {
        Ok(args) => untrack(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

pub(super) async fn untrack(orig: CommandOrigin<'_>, args: TrackArgs) -> Result<()> {
    let TrackArgs {
        name,
        mode,
        mut more_names,
        ..
    } = args;

    more_names.push(name);

    if let Some(name) = more_names.iter().find(|name| name.len() > 15) {
        let content = format!("`{name}` is too long for an osu! username");

        return orig.error(content).await;
    }

    let users = match super::get_names(&more_names, mode.unwrap_or(GameMode::Osu)).await {
        Ok(map) => map,
        Err((OsuError::NotFound, name)) => {
            let content = format!("User `{name}` was not found");

            return orig.error(content).await;
        }
        Err((err, _)) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get names");

            return Err(err);
        }
    };

    let channel = orig.channel_id();
    let mut success = HashSet::with_capacity(users.len());
    let tracking = Context::tracking();

    for (username, user_id) in users {
        let remove_fut = tracking.remove_user(user_id, mode, channel);

        match remove_fut.await {
            Ok(_) => success.insert(username),
            Err(err) => {
                warn!(?err, "Failed to remove tracked entry");

                return send_message(orig, Some(&username), success).await;
            }
        };
    }

    send_message(orig, None, success).await?;

    Ok(())
}

async fn send_message(
    orig: CommandOrigin<'_>,
    name: Option<&Username>,
    success: HashSet<Username>,
) -> Result<()> {
    let success = success.into_iter().collect();
    let embed = UntrackEmbed::new(success, name).build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}

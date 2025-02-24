use std::{collections::HashSet, fmt::Write};

use bathbot_macros::command;
use bathbot_util::{EmbedBuilder, MessageBuilder, constants::GENERAL_ISSUE};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuError};

use super::TrackArgs;
use crate::{
    core::commands::CommandOrigin, manager::redis::osu::UserArgsError, tracking::OsuTracking,
    util::ChannelExt,
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
        Err((UserArgsError::Osu(OsuError::NotFound), name)) => {
            let content = format!("User `{name}` was not found");

            return orig.error(content).await;
        }
        Err((err, _)) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get names");

            return Err(err);
        }
    };

    let channel = orig.channel_id();
    let mut success = HashSet::with_capacity(users.len());

    for (username, user_id) in users {
        OsuTracking::remove_user(user_id, mode, channel).await;
        success.insert(username);
    }

    let mut description = String::new();
    description.push_str("Removed in this channel: ");

    let mut iter = success.iter();

    if let Some(name) = iter.next() {
        let _ = write!(description, "`{name}`");

        for name in iter {
            let _ = write!(description, ", `{name}`");
        }
    } else {
        description.push_str("None");
    }

    let embed = EmbedBuilder::new()
        .title("Top score tracking")
        .description(description);

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}

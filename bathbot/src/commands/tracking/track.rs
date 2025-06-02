use std::fmt::Write;

use bathbot_macros::command;
use bathbot_util::{EmbedBuilder, FooterBuilder, MessageBuilder, constants::GENERAL_ISSUE, fields};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuError};

use super::TrackArgs;
use crate::{
    Context,
    core::commands::CommandOrigin,
    manager::redis::osu::{UserArgsError, UserArgsSlim},
    tracking::{OsuTracking, TrackEntryParams},
    util::{ChannelExt, Emote},
};

pub(super) async fn track(orig: CommandOrigin<'_>, args: TrackArgs) -> Result<()> {
    let TrackArgs {
        name,
        mode,
        mut more_names,
        min_index,
        max_index,
        min_pp,
        max_pp,
        min_combo_percent,
        max_combo_percent,
    } = args;

    more_names.push(name);

    if let Some(name) = more_names.iter().find(|name| name.len() > 15) {
        let content = format!("`{name}` is too long for an osu! username");

        return orig.error(content).await;
    }

    let mode = mode.unwrap_or(GameMode::Osu);

    let users = match super::get_names(&more_names, mode).await {
        Ok(users) => users,
        Err((UserArgsError::Osu(OsuError::NotFound), name)) => {
            let content = format!("User `{name}` was not found");

            return orig.error(content).await;
        }
        Err((err, _)) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get names");

            return Err(err);
        }
    };

    let params = TrackEntryParams::new()
        .with_index(min_index, max_index)
        .with_pp(min_pp, max_pp)
        .with_combo_percent(min_combo_percent, max_combo_percent);

    let channel = orig.channel_id();
    let mut success = Vec::with_capacity(users.len());
    let mut failure = Vec::new();

    for (username, user_id) in users {
        let require = match OsuTracking::add_user(user_id, mode, channel, params).await {
            Ok(Some(require)) => require,
            Ok(None) => {
                success.push(username);

                continue;
            }
            Err(err) => {
                warn!(?err, "Failed to track osu user");
                failure.push(username);

                continue;
            }
        };

        let user_args = UserArgsSlim::user_id(user_id).mode(mode);
        let scores_fut = Context::osu_scores().top(100, false).exec(user_args);

        match scores_fut.await {
            Ok(scores) => match require.callback(&scores).await {
                Ok(()) => success.push(username),
                Err(err) => {
                    warn!(?err, "Failed to track osu user");
                    failure.push(username);
                }
            },
            Err(err) => {
                warn!(?err, "Failed to request top scores to add for tracking");
                failure.push(username);
            }
        }
    }

    let mut fields = Vec::with_capacity(3);
    let mut iter = success.iter();

    if let Some(name) = iter.next() {
        let mut value = String::new();
        let _ = write!(value, "`{name}`");

        for name in iter {
            let _ = write!(value, ", `{name}`");
        }

        fields![fields { "Now tracking:".to_owned(), value, false }];
    }

    let mut iter = failure.iter();

    if let Some(name) = iter.next() {
        let mut value = String::new();
        let _ = write!(value, "`{name}`");

        for name in iter {
            let _ = write!(value, ", `{name}`");
        }

        fields![fields { "Failed to track:".to_owned(), value, false }];
    }

    let value = format!(
        "`Index: {index}` | `PP: {pp}pp` | `Combo percent: {combo_percent}%`",
        index = params.index(),
        pp = params.pp(),
        combo_percent = params.combo_percent(),
    );

    fields![fields { "Parameters:".to_owned(), value, false }];

    let footer = FooterBuilder::new("").icon_url(Emote::from(mode).url());

    let embed = EmbedBuilder::new()
        .fields(fields)
        .footer(footer)
        .title("Top score tracking");

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}

const TRACK_USAGE: &str = "[limit=number] [username1] [username2] ...";

#[command]
#[desc("Track osu!standard user top scores")]
#[help(
    "Track osu!standard user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `track42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `track limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage(TRACK_USAGE)]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Tracking)]
async fn prefix_track(msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Osu), args).await {
        Ok(args) => track(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Track mania user top scores")]
#[help(
    "Track mania user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `trackmania42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `trackmania limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage(TRACK_USAGE)]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Tracking)]
pub async fn prefix_trackmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Mania), args).await {
        Ok(args) => track(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Track taiko user top scores")]
#[help(
    "Track taiko user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `tracktaiko42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `tracktaiko limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage(TRACK_USAGE)]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Tracking)]
pub async fn prefix_tracktaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Taiko), args).await {
        Ok(args) => track(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Track ctb user top scores")]
#[help(
    "Track ctb user top scores and notify a channel \
    about new plays in their top100.\n\
    You can specify __up to ten usernames__ per command invocation.\n\
    To provide a limit, specify a number right after the command, \
    e.g. `trackctb42 badewanne3` to only notify if `badewanne3` got \
    a new score in his top 42.\n\
    Alternatively, you can provide a limit by specifying `limit=number`, \
    e.g. `trackctb limit=42 badewanne3`.\n\
    The limit must be between 1 and 100, **defaults to 50** if none is given."
)]
#[usage(TRACK_USAGE)]
#[examples(
    "badewanne3 \"freddie benson\" peppy limit=23",
    "limit=45 cookiezi whitecat",
    "\"freddie benson\""
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[alias("trackingcatch")]
#[group(Tracking)]
pub async fn prefix_trackctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match TrackArgs::args(Some(GameMode::Catch), args).await {
        Ok(args) => track(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

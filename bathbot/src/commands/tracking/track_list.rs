use std::collections::HashMap;

use bathbot_macros::command;
use bathbot_util::constants::{GENERAL_ISSUE, OSU_API_ISSUE};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuError, Username};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{
    active::{impls::TrackListPagination, ActiveMessages},
    core::commands::CommandOrigin,
    manager::redis::osu::{UserArgs, UserArgsError},
    tracking::{OsuTracking, TrackEntryParams},
    Context,
};

pub struct TracklistUserEntry {
    pub name: Username,
    pub user_id: u32,
    pub mode: GameMode,
    pub params: TrackEntryParams,
}

#[command]
#[desc("Display tracked users of a channel")]
#[alias("tl")]
#[group(Tracking)]
#[flags(AUTHORITY, ONLY_GUILDS)]
async fn prefix_tracklist(msg: &Message) -> Result<()> {
    tracklist(msg.into()).await
}

pub async fn tracklist(orig: CommandOrigin<'_>) -> Result<()> {
    let channel_id = orig.channel_id();

    let entries = match OsuTracking::tracked_users_in_channel(channel_id).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get tracked users"));
        }
    };

    let mut users = match get_users(orig.channel_id(), entries).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get users"));
        }
    };

    users.sort_unstable_by(|a, b| {
        (a.mode as u8)
            .cmp(&(b.mode as u8))
            .then(a.name.cmp(&b.name))
    });

    let pagination = TrackListPagination::builder()
        .entries(users.into_boxed_slice())
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

async fn get_users(
    channel: Id<ChannelMarker>,
    tracked: Vec<(u32, GameMode, TrackEntryParams)>,
) -> Result<Vec<TracklistUserEntry>, UserArgsError> {
    let user_ids: Vec<_> = tracked
        .iter()
        .map(|(user_id, ..)| *user_id as i32)
        .collect();

    // Get all names that are stored in the DB
    let stored_names = match Context::osu_user().names(&user_ids).await {
        Ok(map) => map,
        Err(err) => {
            warn!(?err, "Failed to get names by user ids");

            HashMap::default()
        }
    };

    let mut users = Vec::with_capacity(tracked.len());

    // Get all missing names from the api
    for (user_id, mode, params) in tracked {
        let entry = match stored_names.get(&user_id) {
            Some(name) => TracklistUserEntry {
                name: name.to_owned(),
                user_id,
                mode,
                params,
            },
            None => {
                let user_args = UserArgs::user_id(user_id, mode);

                match Context::redis().osu_user(user_args).await {
                    Ok(user) => TracklistUserEntry {
                        name: user.username.as_str().into(),
                        user_id,
                        mode,
                        params,
                    },
                    Err(UserArgsError::Osu(OsuError::NotFound)) => {
                        OsuTracking::remove_user(user_id, None, channel).await;

                        continue;
                    }
                    Err(err) => return Err(err),
                }
            }
        };

        users.push(entry);
    }

    Ok(users)
}

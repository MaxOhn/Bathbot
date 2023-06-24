use std::{collections::HashMap, sync::Arc};

use bathbot_macros::command;
use bathbot_psql::model::osu::TrackedOsuUserKey;
use bathbot_util::{constants::OSU_API_ISSUE, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMode, OsuError, Username},
    OsuResult,
};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{
    core::commands::CommandOrigin,
    embeds::{EmbedData, TrackListEmbed},
    manager::redis::osu::UserArgs,
    Context,
};

pub struct TracklistUserEntry {
    pub name: Username,
    pub mode: GameMode,
    pub limit: u8,
}

#[command]
#[desc("Display tracked users of a channel")]
#[alias("tl")]
#[group(Tracking)]
#[flags(AUTHORITY, ONLY_GUILDS)]
async fn prefix_tracklist(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    tracklist(ctx, msg.into()).await
}

pub async fn tracklist(ctx: Arc<Context>, orig: CommandOrigin<'_>) -> Result<()> {
    let channel_id = orig.channel_id();
    let tracked = ctx.tracking().list(channel_id).await;

    let mut users = match get_users(&ctx, orig.channel_id(), tracked).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get users"));
        }
    };

    users.sort_unstable_by(|a, b| {
        (a.mode as u8)
            .cmp(&(b.mode as u8))
            .then(a.name.cmp(&b.name))
    });

    let embeds = TrackListEmbed::new(users);

    if embeds.is_empty() {
        let content = "No tracked users in this channel";
        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, builder).await?;
    } else {
        for embed_data in embeds {
            let embed = embed_data.build();
            let builder = MessageBuilder::new().embed(embed);
            orig.create_message(&ctx, builder).await?;
        }
    }

    Ok(())
}

async fn get_users(
    ctx: &Context,
    channel: Id<ChannelMarker>,
    tracked: Vec<(TrackedOsuUserKey, u8)>,
) -> OsuResult<Vec<TracklistUserEntry>> {
    let user_ids: Vec<_> = tracked.iter().map(|(key, ..)| key.user_id as i32).collect();

    // Get all names that are stored in the DB
    let stored_names = match ctx.osu_user().names(&user_ids).await {
        Ok(map) => map,
        Err(err) => {
            warn!(?err, "Failed to get names by user ids");

            HashMap::default()
        }
    };

    let mut users = Vec::with_capacity(tracked.len());

    // Get all missing names from the api
    for (TrackedOsuUserKey { user_id, mode }, limit) in tracked {
        let entry = match stored_names.get(&user_id) {
            Some(name) => TracklistUserEntry {
                name: name.to_owned(),
                mode,
                limit,
            },
            None => {
                let user_args = UserArgs::user_id(user_id).mode(mode);

                match ctx.redis().osu_user(user_args).await {
                    Ok(user) => TracklistUserEntry {
                        name: user.username().into(),
                        mode,
                        limit,
                    },
                    Err(OsuError::NotFound) => {
                        let remove_fut =
                            ctx.tracking()
                                .remove_user(user_id, None, channel, ctx.osu_tracking());

                        if let Err(err) = remove_fut.await {
                            warn!(user_id, ?err, "Failed to remove unknown user from tracking");
                        }

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

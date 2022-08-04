use std::sync::Arc;

use command_macros::command;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::{
    prelude::{GameMode, OsuError, Username},
    OsuResult,
};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{
    core::commands::CommandOrigin,
    embeds::{EmbedData, TrackListEmbed},
    util::{builder::MessageBuilder, constants::OSU_API_ISSUE},
    BotResult, Context,
};

pub struct TracklistUserEntry {
    pub name: Username,
    pub mode: GameMode,
    pub limit: usize,
}

#[command]
#[desc("Display tracked users of a channel")]
#[alias("tl")]
#[group(Tracking)]
#[flags(AUTHORITY, ONLY_GUILDS)]
async fn prefix_tracklist(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    tracklist(ctx, msg.into()).await
}

pub async fn tracklist(ctx: Arc<Context>, orig: CommandOrigin<'_>) -> BotResult<()> {
    let channel_id = orig.channel_id();
    let tracked = ctx.tracking().list(channel_id).await;

    let mut users = match get_users(&ctx, orig.channel_id(), tracked).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
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
        let builder = MessageBuilder::new().content(content);
        orig.create_message(&ctx, &builder).await?;
    } else {
        for embed_data in embeds {
            let embed = embed_data.build();
            let builder = MessageBuilder::new().embed(embed);
            orig.create_message(&ctx, &builder).await?;
        }
    }

    Ok(())
}

async fn get_users(
    ctx: &Context,
    channel: Id<ChannelMarker>,
    tracked: Vec<(u32, GameMode, usize)>,
) -> OsuResult<Vec<TracklistUserEntry>> {
    let user_ids: Vec<_> = tracked.iter().map(|(id, ..)| *id as i32).collect();

    // Get all names that are stored in the DB
    let stored_names = match ctx.psql().get_names_by_ids(&user_ids).await {
        Ok(map) => map,
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to get names by ids");
            warn!("{report:?}",);

            HashMap::default()
        }
    };

    let mut users = Vec::with_capacity(tracked.len());

    // Get all missing names from the api
    for (user_id, mode, limit) in tracked {
        let entry = match stored_names.get(&user_id) {
            Some(name) => TracklistUserEntry {
                name: name.to_owned(),
                mode,
                limit,
            },
            None => match ctx.osu().user(user_id).mode(mode).await {
                Ok(user) => {
                    if let Err(err) = ctx.psql().upsert_osu_user(&user, mode).await {
                        let report = Report::new(err).wrap_err("failed to upsert user");
                        warn!("{report:?}");
                    }

                    TracklistUserEntry {
                        name: user.username,
                        mode,
                        limit,
                    }
                }
                Err(OsuError::NotFound) => {
                    let remove_fut = ctx
                        .tracking()
                        .remove_user(user_id, None, channel, ctx.psql());

                    if let Err(err) = remove_fut.await {
                        warn!("Error while removing unknown user {user_id} from tracking: {err}");
                    }

                    continue;
                }
                Err(err) => return Err(err),
            },
        };

        users.push(entry);
    }

    Ok(users)
}

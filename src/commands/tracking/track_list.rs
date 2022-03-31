use std::sync::Arc;

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::{
    prelude::{GameMode, OsuError, Username},
    OsuResult,
};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{
    embeds::{EmbedData, TrackListEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

pub struct TracklistUserEntry {
    pub name: Username,
    pub mode: GameMode,
    pub limit: usize,
}

#[command]
#[authority()]
#[short_desc("Display tracked users of a channel")]
#[aliases("tl")]
pub async fn tracklist(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let channel_id = data.channel_id();
    let tracked = ctx.tracking().list(channel_id);

    let mut users = match get_users(&ctx, data.channel_id(), tracked).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

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
        data.create_message(&ctx, builder).await?;
    } else {
        for embed_data in embeds {
            let embed = embed_data.into_builder().build();
            let builder = MessageBuilder::new().embed(embed);
            data.create_message(&ctx, builder).await?;
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

    let stored_names = match ctx.psql().get_names_by_ids(&user_ids).await {
        Ok(map) => map,
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to get names by ids");
            warn!("{report:?}",);

            HashMap::new()
        }
    };

    let mut users = Vec::with_capacity(tracked.len());

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

mod acc_lb;
mod join_date_lb;
mod playcount_lb;
mod playtime_lb;
mod pp_lb;
mod ranked_score_lb;
mod total_score_lb;

pub use self::{
    acc_lb::*, join_date_lb::*, playcount_lb::*, playtime_lb::*, pp_lb::*, ranked_score_lb::*,
    total_score_lb::*,
};

use crate::{
    embeds::BasicEmbedData,
    pagination::{Pagination, ReactionData},
    util::numbers,
    DiscordLinks, Error, Osu, TrackTime, TrackedUsers,
};

use chrono::{Duration, Utc};
use futures::StreamExt;
use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, User},
};
use serenity::{
    collector::ReactionAction,
    framework::standard::{macros::group, Args, CommandResult},
    model::{
        channel::{Message, ReactionType},
        id::{ChannelId, GuildId},
    },
    prelude::Context,
};
use std::{convert::TryFrom, sync::Arc};
use tokio::time;

#[group]
#[description = "Commands that can only be used in the belgian osu discord server"]
#[commands(pplb, rankedscore, totalscore, acc, playcount, playtime, joindate)]
struct OsuBelgium;

fn get_mode(mut args: Args) -> GameMode {
    if args.is_empty() {
        GameMode::STD
    } else {
        match args.single::<String>().unwrap().as_str() {
            "mania" | "mna" | "m" => GameMode::MNA,
            "taiko" | "tko" | "t" => GameMode::TKO,
            "fruits" | "ctb" | "c" => GameMode::CTB,
            _ => GameMode::STD,
        }
    }
}

pub async fn member_users(
    ctx: &Context,
    channel: ChannelId,
    guild_id: GuildId,
    mode: GameMode,
) -> Result<(Vec<User>, String), Error> {
    let mut users = {
        let data = ctx.data.read().await;
        let tracked_users = data
            .get::<TrackedUsers>()
            .expect("Could not get TrackedUsers")
            .get(&mode)
            .unwrap();
        if tracked_users.is_empty() {
            None
        } else {
            Some(tracked_users.clone())
        }
    };
    if users.is_none() {
        // Get guild members
        let members: Vec<u64> = {
            let cache = ctx.cache.read().await;
            let cache_guild = cache.guilds.get(&guild_id);
            let guild = cache_guild.unwrap_or_else(|| panic!("Guild not found in cache"));
            let guild = guild.read().await;
            guild.members.keys().map(|id| id.0).collect()
        };

        // Filter members according to osu links
        let names: Vec<String> = {
            let data = ctx.data.read().await;
            let links = data
                .get::<DiscordLinks>()
                .expect("Could not get DiscordLinks");
            members
                .into_iter()
                .filter(|id| links.contains_key(id))
                .map(|id| links.get(&id).unwrap().clone())
                .collect()
        };

        // Request user data
        let msg = channel
            .say(ctx, format!("Requesting {} users...", names.len()))
            .await?;
        debug!("Requesting users for ranked score leaderboard");
        let osu_users = {
            let data = ctx.data.read().await;
            let osu = data.get::<Osu>().expect("Could not get Osu");
            let mut osu_users = Vec::with_capacity(names.len());
            for name in names {
                let req = UserRequest::with_username(&name).mode(mode);
                if let Some(user) = req.queue_single(&osu).await? {
                    osu_users.push(user);
                }
            }
            osu_users
        };

        // Save user data
        {
            let mut data = ctx.data.write().await;
            data.get_mut::<TrackedUsers>()
                .expect("Could not get TrackedUsers")
                .insert(mode, osu_users.clone());
            data.get_mut::<TrackTime>()
                .expect("Could not get TrackTime")
                .insert(mode, Some(Utc::now()));
        }

        // Clear data after an hour
        let data = ctx.data.clone();
        let _ = tokio::spawn(async move {
            time::delay_for(time::Duration::from_secs(3600)).await;
            let mut data = data.write().await;
            data.get_mut::<TrackedUsers>()
                .expect("Could not get TrackedUsers")
                .get_mut(&mode)
                .unwrap()
                .clear();
        });
        let _ = msg.delete(ctx).await;
        users = Some(osu_users);
    }
    let track_time = {
        let data = ctx.data.read().await;
        data.get::<TrackTime>()
            .expect("Could not get TrackTime")
            .get(&mode)
            .unwrap()
            .unwrap()
    };
    let update_interval = Duration::hours(1);
    let until = track_time + update_interval - Utc::now();
    let next_update = if until.num_minutes() > 0 {
        format!("{} minutes", until.num_minutes())
    } else {
        format!("{} seconds", until.num_seconds())
    };
    Ok((users.unwrap(), next_update))
}

pub async fn send_response(
    ctx: &Context,
    list: Vec<(String, String)>,
    next_update: String,
    msg: &Message,
) -> CommandResult {
    // Prepare initial data
    let initial_data = list
        .iter()
        .take(15)
        .map(|(name, score)| (name, score))
        .collect();
    let pages = numbers::div_euclid(15, list.len());
    let data =
        BasicEmbedData::create_belgian_leaderboard(&next_update, initial_data, 1, (1, pages));

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(ctx)
        .timeout(std::time::Duration::from_secs(60))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        let reaction_type = ReactionType::try_from(reaction).unwrap();
        response.react(&ctx.http, reaction_type).await?;
    }
    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    tokio::spawn(async move {
        let mut pagination = Pagination::belgian_lb(next_update, list);
        while let Some(reaction) = collector.next().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction) = &reaction.emoji {
                    match pagination.next_reaction(reaction.as_str()).await {
                        Ok(data) => match data {
                            ReactionData::Delete => response.delete((&cache, &*http)).await?,
                            ReactionData::None => {}
                            _ => {
                                response
                                    .edit((&cache, &*http), |m| m.embed(|e| data.build(e)))
                                    .await?
                            }
                        },
                        Err(why) => warn!(
                            "Error while using paginator for belgian leaderboard: {}",
                            why
                        ),
                    }
                }
            }
        }

        // Remove initial reactions
        for &reaction in reactions.iter() {
            let reaction_type = ReactionType::try_from(reaction).unwrap();
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction_type)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

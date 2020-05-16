use crate::{embeds::BasicEmbedData, util::numbers, DiscordLinks, Osu, TrackTime, TrackedUsers};

use crate::{
    commands::checks::*,
    pagination::{Pagination, ReactionData},
};

use chrono::{Duration, Utc};
use futures::StreamExt;
use rosu::{backend::requests::UserRequest, models::GameMode};
use serenity::{
    collector::ReactionAction,
    framework::standard::{macros::command, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{convert::TryFrom, sync::Arc};
use tokio::time;

#[command]
#[checks(MainGuild)]
#[description = "Show the ranked score leaderboard among all linked members in this server"]
async fn rankedscore(ctx: &Context, msg: &Message) -> CommandResult {
    let mut users = {
        let data = ctx.data.read().await;
        let tracked_users = data
            .get::<TrackedUsers>()
            .expect("Could not get TrackedUsers");
        if tracked_users.is_empty() {
            None
        } else {
            Some(tracked_users.clone())
        }
    };
    if users.is_none() {
        // Get guild members
        let members: Vec<u64> = {
            let guild_id = msg.guild_id.unwrap();
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
        debug!("Requesting users for ranked score leaderboard");
        let osu_users = {
            let data = ctx.data.read().await;
            let osu = data.get::<Osu>().expect("Could not get Osu");
            let mut osu_users = Vec::with_capacity(names.len());
            for name in names {
                let req = UserRequest::with_username(&name).mode(GameMode::STD); // TODO: Handle mode
                if let Some(user) = req.queue_single(&osu).await? {
                    osu_users.push(user);
                }
            }
            osu_users
        };

        // Save user data
        {
            let mut data = ctx.data.write().await;
            let tracked_users = data
                .get_mut::<TrackedUsers>()
                .expect("Could not get TrackedUsers");
            *tracked_users = osu_users.clone();
            let track_time = data
                .get_mut::<TrackTime>()
                .expect("Could not get TrackTime");
            *track_time = Some(Utc::now());
        }

        // Clear data after an hour
        let data = ctx.data.clone();
        let _ = tokio::spawn(async move {
            time::delay_for(time::Duration::from_secs(3600)).await;
            let mut data = data.write().await;
            data.get_mut::<TrackedUsers>()
                .expect("Could not get TrackedUsers")
                .clear();
        });
        users = Some(osu_users);
    }
    let track_time = {
        let data = ctx.data.read().await;
        data.get::<TrackTime>()
            .expect("Could not get TrackTime")
            .unwrap()
    };
    let mut users: Vec<_> = users
        .unwrap()
        .into_iter()
        .map(|u| (u.username, u.ranked_score))
        .collect();
    users.sort_by(|(_, a), (_, b)| b.cmp(&a));

    // Prepare initial data
    let update_interval = Duration::hours(1);
    let until = track_time + update_interval - Utc::now();
    let next_update = if until.num_minutes() > 0 {
        format!("{} minutes", until.num_minutes())
    } else {
        format!("{} seconds", until.num_seconds())
    };
    let initial_data = users
        .iter()
        .take(15)
        .map(|(name, score)| (name, *score))
        .collect();
    let pages = numbers::div_euclid(15, users.len());
    let data = BasicEmbedData::create_ranked_score(&next_update, initial_data, 1, (1, pages));

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(&ctx)
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
        let mut pagination = Pagination::ranked_score(next_update, users);
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
                        Err(why) => warn!("Error while using paginator for rankedscore: {}", why),
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

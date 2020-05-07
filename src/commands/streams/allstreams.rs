use crate::{embeds::BasicEmbedData, util::discord};

use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::{gateway::ActivityType, prelude::Message},
    prelude::Context,
};
use std::{collections::HashMap, sync::Arc};

#[command]
#[only_in("guild")]
#[description = "List all members of this server that are currently streaming"]
#[aliases("allstreamers", "streams")]
async fn allstreams(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let presences: Vec<_> = {
        let cache = ctx.cache.clone();
        let http = Arc::clone(&ctx.http);
        let guild_lock = guild_id.to_guild_cached(&cache).await.unwrap();
        let guild = guild_lock.read().await;
        let mut presences = Vec::with_capacity(guild.presences.len());
        for (_, presence) in guild.presences.iter() {
            if let Some(activity) = presence.activity.as_ref() {
                if activity.kind == ActivityType::Streaming
                    && !presence
                        .user_id
                        .to_user((&cache, &*http))
                        .await
                        .unwrap()
                        .bot
                {
                    presences.push(presence.clone());
                }
            }
        }
        presences
    };
    let total = presences.len();
    let presences: Vec<_> = presences.into_iter().take(60).collect();
    let avatar = guild_id
        .to_guild_cached(&ctx.cache)
        .await
        .unwrap_or_else(|| panic!("Guild {} not found in cache", guild_id))
        .read()
        .await
        .icon_url();
    let mut users = HashMap::with_capacity(presences.len());
    for presence in presences.iter() {
        users.insert(
            presence.user_id,
            presence.user_id.to_user(ctx).await.unwrap().name,
        );
    }

    // Accumulate all necessary data
    let data = BasicEmbedData::create_allstreams(presences, users, total, avatar);

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}

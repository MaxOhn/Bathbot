use crate::{
    embeds::{AllStreamsEmbed, EmbedData},
    util::MessageExt,
};

use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::{gateway::ActivityType, prelude::Message},
    prelude::Context,
};
use std::collections::HashMap;

#[command]
#[only_in("guild")]
#[description = "List all members of this server that are currently streaming"]
#[aliases("allstreamers", "streams")]
async fn allstreams(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let presences: Vec<_> = {
        let guild = guild_id.to_guild_cached(ctx).await.unwrap();
        let mut presences = Vec::with_capacity(guild.presences.len());
        for (_, presence) in guild.presences.iter() {
            if let Some(activity) = presence.activity.as_ref() {
                if activity.kind == ActivityType::Streaming
                    && !presence.user_id.to_user(ctx).await.unwrap().bot
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
        .icon_url();
    let mut users = HashMap::with_capacity(presences.len());
    for presence in presences.iter() {
        users.insert(
            presence.user_id,
            presence.user_id.to_user(ctx).await.unwrap().name,
        );
    }

    // Accumulate all necessary data
    let data = AllStreamsEmbed::new(presences, users, total, avatar);

    // Creating the embed
    msg.channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}

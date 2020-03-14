use crate::{embeds::BasicEmbedData, util::discord};

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
fn allstreams(ctx: &mut Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let presences: Vec<_> = {
        let guild_lock = guild_id.to_guild_cached(&ctx.cache).unwrap();
        let guild = guild_lock.read();
        guild
            .presences
            .iter()
            .filter(|(_, presence)| match &presence.activity {
                Some(activity) => {
                    activity.kind == ActivityType::Streaming
                        && !presence.user_id.to_user(&ctx).unwrap().bot
                }
                None => false,
            })
            .map(|(_, presence)| presence.clone())
            .collect()
    };
    let total = presences.len();
    let presences: Vec<_> = presences.into_iter().take(60).collect();
    let avatar = guild_id
        .to_guild_cached(&ctx.cache)
        .unwrap_or_else(|| panic!("Guild {} not found in cache", guild_id))
        .read()
        .icon_url()
        .clone();
    let users: HashMap<_, _> = presences
        .iter()
        .map(|p| (p.user_id, p.user_id.to_user(&ctx).unwrap().name))
        .collect();

    // Accumulate all necessary data
    let data = BasicEmbedData::create_allstreams(presences, users, total, avatar);

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

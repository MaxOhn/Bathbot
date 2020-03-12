use crate::{embeds::BasicEmbedData, util::discord};

use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::{gateway::ActivityType, prelude::Message},
    prelude::Context,
};

#[command]
#[only_in("guild")]
#[description = "List all members of this server that are currently streaming"]
#[aliases("allstreamers", "streams")]
fn allstreams(ctx: &mut Context, msg: &Message) -> CommandResult {
    let presences: Vec<_> = {
        let guild_lock = msg.guild_id.unwrap().to_guild_cached(&ctx.cache).unwrap();
        let guild = guild_lock.read();
        guild
            .presences
            .iter()
            .filter(|(_, presence)| match &presence.activity {
                Some(activity) => {
                    activity.kind == ActivityType::Streaming
                        && !presence.user.as_ref().unwrap().read().bot
                }
                None => false,
            })
            .map(|(_, presence)| presence.clone())
            .collect()
    };
    let total = presences.len();
    let presences = presences.into_iter().take(60).collect();
    let avatar = ctx.cache.read().user.avatar_url().unwrap();

    // Accumulate all necessary data
    let data = BasicEmbedData::create_allstreams(presences, total, avatar);

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

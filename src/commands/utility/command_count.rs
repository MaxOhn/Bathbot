use crate::{
    embeds::BasicEmbedData,
    pagination::{Pagination, ReactionData},
    util::datetime,
    util::numbers,
    BootTime, CommandCounter,
};

use chrono::{DateTime, Utc};
use futures::StreamExt;
use serenity::{
    collector::ReactionAction,
    framework::standard::{macros::command, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{sync::Arc, time::Duration};

#[command]
#[description = "Let me show you my most popular commands \
                 since my last reboot"]
async fn commands(ctx: &mut Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let counter = data
        .get::<CommandCounter>()
        .expect("Could not get CommandCounter");
    let mut vec = Vec::with_capacity(counter.len());
    for (command, amount) in counter {
        vec.push((command.clone(), *amount));
    }
    vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));

    // Prepare embed data
    let boot_time = {
        let boot_time: &DateTime<Utc> = data.get::<BootTime>().expect("Could not get BootTime");
        datetime::how_long_ago(boot_time)
    };
    let sub_vec = vec
        .iter()
        .take(15)
        .map(|(name, amount)| (name, *amount))
        .collect();
    let pages = numbers::div_euclid(15, vec.len());
    let data = BasicEmbedData::create_command_counter(sub_vec, boot_time.as_str(), 1, (1, pages));

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(&ctx)
        .timeout(Duration::from_secs(60))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        response.react(&ctx.http, reaction).await?;
    }
    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    tokio::spawn(async move {
        let mut pagination = Pagination::command_counter(vec, boot_time);
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
                        Err(why) => {
                            warn!("Error while using paginator for command counter: {}", why)
                        }
                    }
                }
            }
        }

        // Remove initial reactions
        for &reaction in reactions.iter() {
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

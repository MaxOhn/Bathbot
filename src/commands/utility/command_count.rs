use crate::{
    embeds::BasicEmbedData,
    pagination::{CommandCountPagination, Pagination},
    util::datetime,
    util::numbers,
    BootTime, CommandCounter,
};

use chrono::{DateTime, Utc};
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{convert::TryFrom, sync::Arc, time::Duration};
use tokio::stream::StreamExt;

#[command]
#[description = "Let me show you my most popular commands \
                 since my last reboot"]
async fn commands(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let counter = data.get::<CommandCounter>().unwrap();
    let mut vec = Vec::with_capacity(counter.len());
    for (command, amount) in counter {
        vec.push((command.clone(), *amount));
    }
    vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));

    // Prepare embed data
    let boot_time = {
        let boot_time: &DateTime<Utc> = data.get::<BootTime>().unwrap();
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
    let mut resp = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Collect reactions of author on the response
    let mut collector = resp
        .await_reactions(&ctx)
        .timeout(Duration::from_secs(60))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        let reaction_type = ReactionType::try_from(reaction).unwrap();
        resp.react(&ctx.http, reaction_type).await?;
    }
    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = Arc::clone(&ctx.cache);
    tokio::spawn(async move {
        let mut pagination = CommandCountPagination::new(vec, boot_time);
        while let Some(reaction) = collector.next().await {
            match pagination.next_page(reaction, &resp, &cache, &http).await {
                Ok(Some(data)) => {
                    resp.edit((&cache, &*http), |m| m.embed(|e| data.build(e)))
                        .await?;
                }
                Ok(None) => {}
                Err(why) => warn!("Error while using CommandCountPagination: {}", why),
            }
        }

        // Remove initial reactions
        for &reaction in reactions.iter() {
            let reaction_type = ReactionType::try_from(reaction).unwrap();
            resp.channel_id
                .delete_reaction(&http, resp.id, None, reaction_type)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

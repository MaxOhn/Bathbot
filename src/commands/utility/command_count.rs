use crate::{
    embeds::{CommandCounterEmbed, EmbedData},
    pagination::{CommandCountPagination, Pagination},
    util::datetime,
    util::numbers,
    BootTime, CommandCounter,
};

use chrono::{DateTime, Utc};
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::channel::Message,
    prelude::Context,
};
use std::sync::Arc;

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
    let data = CommandCounterEmbed::new(sub_vec, boot_time.as_str(), 1, (1, pages));

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Pagination
    let pagination = CommandCountPagination::new(ctx, resp, msg.author.id, vec, boot_time).await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

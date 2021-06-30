use crate::{
    embeds::{CommandCounterEmbed, EmbedData},
    pagination::{CommandCountPagination, Pagination},
    util::{numbers, MessageExt},
    Args, BotResult, Context,
};

use prometheus::core::Collector;
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("List of popular commands")]
#[long_desc("Let me show you my most popular commands since my last reboot")]
async fn commands(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let mut cmds: Vec<_> = ctx.stats.command_counts.collect()[0]
        .get_metric()
        .iter()
        .map(|metric| {
            let name = metric.get_label()[0].get_value();
            let count = metric.get_counter().get_value();

            (name.to_owned(), count as u32)
        })
        .collect();

    cmds.sort_unstable_by(|&(_, a), &(_, b)| b.cmp(&a));

    // Prepare embed data
    let boot_time = ctx.stats.start_time;
    let sub_vec = cmds
        .iter()
        .take(15)
        .map(|(name, amount)| (name, *amount))
        .collect();

    let pages = numbers::div_euclid(15, cmds.len());

    let embed = CommandCounterEmbed::new(sub_vec, &boot_time, 1, (1, pages))
        .into_builder()
        .build();

    // Creating the embed
    let response = msg.respond_embed(&ctx, embed).await?;

    // Pagination
    let pagination = CommandCountPagination::new(&ctx, response, cmds);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 90).await {
            unwind_error!(warn, why, "Pagination error (command count): {}")
        }
    });

    Ok(())
}

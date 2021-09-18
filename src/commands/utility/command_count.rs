use crate::{
    commands::SlashCommandBuilder,
    embeds::{CommandCounterEmbed, EmbedData},
    pagination::{CommandCountPagination, Pagination},
    util::{numbers, MessageExt},
    BotResult, CommandData, Context,
};

use prometheus::core::Collector;
use std::sync::Arc;
use twilight_model::application::{command::Command, interaction::ApplicationCommand};

#[command]
#[short_desc("List of popular commands")]
#[long_desc("Let me show you my most popular commands since my last reboot")]
async fn commands(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let owner = data.author()?.id;

    let mut cmds: Vec<_> = ctx.stats.command_counts.message_commands.collect()[0]
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

    // Creating the embed
    let embed_data = CommandCounterEmbed::new(sub_vec, &boot_time, 1, (1, pages));
    let builder = embed_data.into_builder().build().into();
    let response = data.create_message(&ctx, builder).await?.model().await?;

    // Pagination
    let pagination = CommandCountPagination::new(&ctx, response, cmds);

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 90).await {
            unwind_error!(warn, why, "Pagination error (command count): {}")
        }
    });

    Ok(())
}

pub async fn slash_commands(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    commands(ctx, command.into()).await
}

pub fn slash_commands_command() -> Command {
    let description = "Display a list of popular commands";

    SlashCommandBuilder::new("commands", description).build()
}

use std::sync::Arc;

use command_macros::{command, SlashCommand};
use prometheus::core::Collector;
use twilight_interactions::command::CreateCommand;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::commands::CommandOrigin,
    embeds::{CommandCounterEmbed, EmbedData},
    pagination::{CommandCountPagination, Pagination},
    util::numbers,
    BotResult, Context,
};

#[derive(CreateCommand, SlashCommand)]
#[command(name = "commands")]
#[flags(SKIP_DEFER)]
/// Display a list of popular commands
pub struct Commands;

pub async fn slash_commands(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    commands(ctx, command.into()).await
}

#[command]
#[desc("List of popular commands")]
#[group(Utility)]
#[flags(SKIP_DEFER)]
async fn prefix_commands(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    commands(ctx, msg.into()).await
}

async fn commands(ctx: Arc<Context>, orig: CommandOrigin<'_>) -> BotResult<()> {
    let owner = orig.user_id()?;

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
    let builder = embed_data.build().into();
    let response = orig
        .callback_with_response(&ctx, builder)
        .await?
        .model()
        .await?;

    // Pagination
    CommandCountPagination::new(&ctx, response, cmds).start(ctx, owner, 60);

    Ok(())
}

use std::sync::Arc;

use bathbot_macros::{command, SlashCommand};
use bathbot_model::{RankingEntries, RankingEntry, RankingKind};
use eyre::Result;
use prometheus::core::Collector;
use twilight_interactions::command::CreateCommand;

use crate::{
    core::commands::CommandOrigin, pagination::RankingPagination,
    util::interaction::InteractionCommand, Context,
};

#[derive(CreateCommand, SlashCommand)]
#[command(name = "commands", desc = "Display a list of popular prefix commands")]
#[flags(SKIP_DEFER)]
pub struct Commands;

pub async fn slash_commands(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    commands(ctx, (&mut command).into()).await
}

#[command]
#[desc("List of popular prefix commands")]
#[group(Utility)]
#[flags(SKIP_DEFER)]
async fn prefix_commands(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    commands(ctx, msg.into()).await
}

async fn commands(ctx: Arc<Context>, orig: CommandOrigin<'_>) -> Result<()> {
    let mut cmds: Vec<_> = ctx.stats.command_counts.prefix_commands.collect()[0]
        .get_metric()
        .iter()
        .map(|metric| {
            let name = metric.get_label()[0].get_value();
            let count = metric.get_counter().get_value();

            (name.to_owned(), count as u32)
        })
        .collect();

    cmds.sort_unstable_by(|&(_, a), &(_, b)| b.cmp(&a));

    let entries = cmds
        .into_iter()
        .enumerate()
        .map(|(i, (name, count))| {
            let entry = RankingEntry {
                country: None,
                name: name.into(),
                value: count as u64,
            };

            (i, entry)
        })
        .collect();

    let entries = RankingEntries::Amount(entries);
    let total = entries.len();

    let kind = RankingKind::Commands {
        bootup_time: ctx.stats.start_time,
    };

    RankingPagination::builder(entries, total, None, kind)
        .start(ctx, orig)
        .await
}

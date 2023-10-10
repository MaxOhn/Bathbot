use std::{cmp::Reverse, sync::Arc};

use bathbot_macros::{command, SlashCommand};
use bathbot_model::{RankingEntries, RankingEntry, RankingKind};
use eyre::Result;
use prometheus::core::Collector;
use twilight_interactions::command::CreateCommand;

use crate::{
    active::{
        impls::{RankingPagination, SlashCommandsPagination},
        ActiveMessages,
    },
    core::commands::CommandOrigin,
    util::{interaction::InteractionCommand, Authored},
    Context,
};

#[derive(CreateCommand, SlashCommand)]
#[command(name = "commands", desc = "Display a list of popular slash commands")]
#[flags(SKIP_DEFER)]
pub struct Commands;

pub async fn slash_commands(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let owner = command.user_id()?;

    let mut cmd_counts: Box<[_]> = ctx.stats.command_counts.slash_commands.collect()[0]
        .get_metric()
        .iter()
        .map(|metric| {
            let [label1, label2, label3] = metric.get_label() else {
                unreachable!()
            };

            let mut name = "";
            let mut group = "";
            let mut sub = "";

            macro_rules! assign_val {
                ( $( $label:ident ),* ) => {
                    $(
                        match $label.get_name() {
                            "name" => name = $label.get_value(),
                            "group" => group = $label.get_value(),
                            "sub" => sub = $label.get_value(),
                            _ => unreachable!(),
                        }
                    )*
                };
            }

            assign_val!(label1, label2, label3);

            let mut cmd = String::with_capacity(
                name.len()
                    + (!group.is_empty() as usize + group.len())
                    + (!sub.is_empty() as usize + sub.len()),
            );

            cmd.push_str(name);

            if !group.is_empty() {
                cmd.push(' ');
                cmd.push_str(group);
            }

            if !sub.is_empty() {
                cmd.push(' ');
                cmd.push_str(sub);
            }

            let count = metric.get_counter().get_value();

            (cmd.into_boxed_str(), count as u32)
        })
        .collect();

    cmd_counts.sort_unstable_by(|(name_a, count_a), (name_b, count_b)| {
        count_b.cmp(count_a).then_with(|| name_a.cmp(name_b))
    });

    let pagination = SlashCommandsPagination::builder()
        .counts(cmd_counts)
        .start_time(ctx.stats.start_time)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(false)
        .begin(ctx, &mut command)
        .await
}

#[command]
#[desc("List of popular prefix commands")]
#[group(Utility)]
#[flags(SKIP_DEFER)]
async fn prefix_commands(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    let orig = CommandOrigin::from(msg);

    let mut cmds: Vec<_> = ctx.stats.command_counts.prefix_commands.collect()[0]
        .get_metric()
        .iter()
        .map(|metric| {
            let name = metric.get_label()[0].get_value();
            let count = metric.get_counter().get_value();

            (name.to_owned(), count as u32)
        })
        .collect();

    cmds.sort_unstable_by_key(|(_, count)| Reverse(*count));

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

    let msg_owner = orig.user_id()?;
    let entries = RankingEntries::Amount(entries);
    let total = entries.len();

    let kind = RankingKind::Commands {
        bootup_time: ctx.stats.start_time,
    };

    let pagination = RankingPagination::builder()
        .entries(entries)
        .total(total)
        .kind(kind)
        .defer(false)
        .msg_owner(msg_owner)
        .build();

    ActiveMessages::builder(pagination).begin(ctx, orig).await
}

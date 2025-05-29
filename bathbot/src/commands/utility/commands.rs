use std::{cmp::Reverse, slice};

use bathbot_macros::{SlashCommand, command};
use bathbot_model::{RankingEntries, RankingEntry, RankingKind};
use bathbot_util::Authored;
use eyre::Result;
use metrics::{Key, Label};
use twilight_interactions::command::CreateCommand;

use crate::{
    Context,
    active::{
        ActiveMessages,
        impls::{RankingPagination, SlashCommandsPagination},
    },
    core::commands::CommandOrigin,
    util::interaction::InteractionCommand,
};

#[derive(CreateCommand, SlashCommand)]
#[command(name = "commands", desc = "Display a list of popular slash commands")]
#[flags(SKIP_DEFER)]
pub struct Commands;

pub async fn slash_commands(mut command: InteractionCommand) -> Result<()> {
    static LABEL: Label = Label::from_static_parts("kind", "slash");

    let owner = command.user_id()?;

    let key = Key::from_static_parts("bathbot.commands_process_time", slice::from_ref(&LABEL));

    let mut full_name = String::new();

    let mut cmd_counts: Box<[(Box<str>, u32)]> = Context::get()
        .metrics
        .collect_histograms(&key, |key, count| {
            full_name.clear();

            let name = key
                .labels()
                .find_map(|label| (label.key() == "name").then_some(label.value()))
                .unwrap_or("<unknown name>");

            full_name.push_str(name);

            let group = key
                .labels()
                .find_map(|label| (label.key() == "group").then_some(label.value()));

            if let Some(group) = group.filter(|group| !group.is_empty()) {
                full_name.push(' ');
                full_name.push_str(group);
            }

            let sub = key
                .labels()
                .find_map(|label| (label.key() == "sub").then_some(label.value()));

            if let Some(sub) = sub.filter(|sub| !sub.is_empty()) {
                full_name.push(' ');
                full_name.push_str(sub);
            }

            (Box::from(full_name.as_str()), count as u32)
        })
        .into_boxed_slice();

    cmd_counts.sort_unstable_by(|(name_a, count_a), (name_b, count_b)| {
        count_b.cmp(count_a).then_with(|| name_a.cmp(name_b))
    });

    let pagination = SlashCommandsPagination::builder()
        .counts(cmd_counts)
        .start_time(Context::get().start_time)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(false)
        .begin(&mut command)
        .await
}

#[command]
#[desc("List of popular prefix commands")]
#[group(Utility)]
#[flags(SKIP_DEFER)]
async fn prefix_commands(msg: &Message) -> Result<()> {
    static LABEL: Label = Label::from_static_parts("kind", "prefix");

    let orig = CommandOrigin::from(msg);

    let key = Key::from_static_parts("bathbot.commands_process_time", slice::from_ref(&LABEL));

    let mut cmds = Context::get()
        .metrics
        .collect_histograms(&key, |key, count| {
            let name: Box<str> = key
                .labels()
                .find_map(|label| (label.key() == "name").then(|| Box::from(label.value())))
                .unwrap_or_else(|| Box::from("<unknown name>"));

            (name, count as u32)
        });

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
        bootup_time: Context::get().start_time,
    };

    let pagination = RankingPagination::builder()
        .entries(entries)
        .total(total)
        .kind(kind)
        .defer(false)
        .msg_owner(msg_owner)
        .build();

    ActiveMessages::builder(pagination).begin(orig).await
}

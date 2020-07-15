use crate::{
    core::{CommandGroups, MessageExt},
    util::{
        constants::{DARK_GREEN, RED},
        levenshtein_distance,
    },
    BotResult, Context,
};

use std::{collections::BTreeMap, fmt::Write};
use twilight::{builders::embed::EmbedBuilder, model::channel::Message};

pub async fn failed_help(
    ctx: &Context,
    arg: String,
    cmds: &CommandGroups,
    msg: &Message,
) -> BotResult<()> {
    let mut dists = BTreeMap::new();
    let names = cmds
        .groups
        .iter()
        .flat_map(|group| group.commands.iter().flat_map(|cmd| cmd.names.iter()));
    for name in names {
        let dist = levenshtein_distance(&arg, name);
        if dist < 4 {
            dists.insert(dist, name);
        }
    }
    let (content, color) = if dists.is_empty() {
        (String::from("There is no such command"), RED)
    } else {
        let mut names = dists.iter().take(5).map(|(_, name)| name);
        let count = dists.len().min(5);
        let mut content = String::with_capacity(14 + count * (4 + 2) + (count - 1) * 2);
        content.push_str("Did you mean ");
        write!(content, "`{}`", names.next().unwrap())?;
        for name in names {
            write!(content, ", `{}`", name)?;
        }
        content.push('?');
        (content, DARK_GREEN)
    };
    msg.build_response(ctx, |m| {
        let embed = EmbedBuilder::new()
            .description(content)
            .color(color)
            .build();
        m.embed(embed)
    })
    .await?;
    Ok(())
}

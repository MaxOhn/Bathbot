mod interaction;
mod message;

pub use interaction::{define_help, handle_menu_select, slash_help};
pub use message::{failed_help, help, help_command};

use std::{collections::BTreeMap, fmt::Write};

use crate::{
    core::{commands::CommandDataCompact, Context},
    embeds::EmbedBuilder,
    util::{constants::RED, MessageBuilder, MessageExt},
    BotResult,
};

async fn failed_message_(
    ctx: &Context,
    data: CommandDataCompact,
    dists: BTreeMap<usize, &'static str>,
) -> BotResult<()> {
    // Needs tighter scope for some reason or tokio complains about something being not `Send`
    let content = {
        let mut names = dists.iter().take(5).map(|(_, &name)| name);

        if let Some(name) = names.next() {
            let count = dists.len().min(5);
            let mut content = String::with_capacity(14 + count * (5 + 2) + (count - 1) * 2);
            content.push_str("Did you mean ");
            write!(content, "`{name}`")?;

            for name in names {
                write!(content, ", `{name}`")?;
            }

            content.push('?');

            content
        } else {
            "There is no such command".to_owned()
        }
    };

    let embed = EmbedBuilder::new().description(content).color(RED).build();

    let builder = MessageBuilder::new().embed(embed);
    data.create_message(ctx, builder).await?;

    Ok(())
}

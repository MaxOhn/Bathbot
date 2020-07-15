use crate::{
    core::{Command, MessageExt},
    util::constants::DARK_GREEN,
    BotResult, Context,
};

use std::fmt::Write;
use twilight::{builders::embed::EmbedBuilder, model::channel::Message};

pub async fn help_command(ctx: &Context, cmd: &Command, msg: &Message) -> BotResult<()> {
    msg.build_response(ctx, |m| {
        let name = cmd.names[0];
        let mut eb = EmbedBuilder::new().color(DARK_GREEN).title(name);
        if let Some(description) = cmd.long_desc {
            eb = eb.description(description);
        }
        if let Some(usage) = cmd.usage {
            eb = eb.add_field("How to use", usage).inline().commit();
        }
        if !cmd.examples.is_empty() {
            let len: usize = cmd.examples.iter().map(|e| name.len() + e.len() + 4).sum();
            let mut value = String::with_capacity(len);
            let mut examples = cmd.examples.iter();
            writeln!(value, "`{} {}`", name, examples.next().unwrap());
            for example in examples {
                writeln!(value, "`{} {}`", name, example);
            }
            eb = eb.add_field("Examples", value).inline().commit();
        }
        if cmd.names.len() > 1 {
            let len: usize = cmd.names.iter().skip(1).map(|n| 4 + n.len()).sum();
            let mut value = String::with_capacity(len);
            let mut aliases = cmd.names.iter().skip(1);
            write!(value, "`{}`", aliases.next().unwrap());
            for alias in aliases {
                write!(value, ", `{}`", alias);
            }
            eb = eb.add_field("Aliases", value).inline().commit();
        }
        m.embed(eb.build())
    })
    .await?;
    Ok(())
}

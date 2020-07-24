use crate::{
    arguments::Args,
    bail,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::{cmp::Ordering, fmt::Write, sync::Arc};
use twilight::model::channel::Message;

#[command]
#[short_desc("Change my prefixes for a guild")]
#[long_desc(
    "Change my prefixes for a guild.\n\
    To check the current prefixes for this guild, \
    don't pass any arguments.\n\
    Otherwise, the first argument must be either `add` or `remove`.\n\
    Following that must be a space-separated list of \
    characters or strings you want to add or remove as prefix.\n\
    Guilds must have between one and five prefixes."
)]
#[authority()]
#[usage("[add / remove] [prefix]")]
#[example("add $ new_pref :eggplant:")]
#[example("remove < !!")]
#[aliases("prefixes")]
async fn prefix(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let mut args = Args::new(msg.content.clone());
    let guild_id = msg.guild_id.unwrap();
    if args.is_empty() {
        let guard = match ctx.guilds().get(&guild_id) {
            Some(guard) => guard,
            None => {
                msg.respond(&ctx, GENERAL_ISSUE).await?;
                bail!("No config for guild {}", guild_id);
            }
        };
        let config = guard.value();
        let mut content = String::new();
        current_prefixes(&mut content, &config.prefixes);
        msg.respond(&ctx, content).await?;
        return Ok(());
    }
    let action = match args.single::<String>().unwrap().as_str() {
        "add" | "a" => Action::Add,
        "remove" | "r" => Action::Remove,
        other => {
            let content = format!(
                "If any arguments are provided, the first one \
                must be either `add` or `remove`, not `{}`",
                other
            );
            msg.respond(&ctx, content).await?;
            return Ok(());
        }
    };
    if args.is_empty() {
        let content = "After the first argument you should specify some prefix(es)";
        msg.respond(&ctx, content).await?;
        return Ok(());
    }
    let config = ctx.guilds().update_get(&guild_id, |_, config| {
        let mut new_config = config.clone();
        let args = args.iter().take(5);
        match action {
            Action::Add => {
                new_config.prefixes.extend(args.map(|arg| arg.to_owned()));
                new_config.prefixes.sort_unstable_by(|a, b| {
                    if a == "<" {
                        Ordering::Less
                    } else if b == "<" {
                        Ordering::Greater
                    } else {
                        a.cmp(&b)
                    }
                });
                new_config.prefixes.dedup();
                new_config.prefixes.truncate(5);
            }
            Action::Remove => {
                for arg in args {
                    new_config.prefixes.retain(|p| p.as_str() != arg);
                    if new_config.prefixes.is_empty() {
                        new_config.prefixes.push(arg.to_owned());
                        break;
                    }
                }
            }
        }
        new_config
    });
    if let Some(config) = config {
        let mut content = "Prefixes updated!\n".to_owned();
        current_prefixes(&mut content, &config.prefixes);
        msg.respond(&ctx, content).await?;
        Ok(())
    } else {
        msg.respond(&ctx, GENERAL_ISSUE).await?;
        bail!("Unsuccessful update of guild prefixes");
    }
}

enum Action {
    Add,
    Remove,
}

fn current_prefixes(content: &mut String, prefixes: &[String]) {
    content.push_str("Prefixes for this guild: ");
    let len = prefixes.iter().map(|p| p.len() + 4).sum();
    content.reserve_exact(len);
    let mut prefixes = prefixes.iter();
    let _ = write!(content, "`{}`", prefixes.next().unwrap());
    for prefix in prefixes {
        let _ = write!(content, ", {}", prefix);
    }
}

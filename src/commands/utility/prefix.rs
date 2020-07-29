use crate::{
    bail,
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
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
#[only_guilds()]
#[authority()]
#[usage("[add / remove] [prefix]")]
#[example("add $ new_pref :eggplant:")]
#[example("remove < !!")]
#[aliases("prefixes")]
async fn prefix(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let guild_id = msg.guild_id.unwrap();
    if args.is_empty() {
        let prefixes = ctx.config_prefixes(guild_id);
        let mut content = String::new();
        current_prefixes(&mut content, &prefixes);
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
            return msg.error(&ctx, content).await;
        }
    };
    if args.is_empty() {
        let content = "After the first argument you should specify some prefix(es)";
        return msg.error(&ctx, content).await;
    }
    ctx.update_config(guild_id, |config| {
        let args = args.take(5);
        match action {
            Action::Add => {
                config.prefixes.extend(args.map(|arg| arg.to_owned()));
                config.prefixes.sort_unstable_by(|a, b| {
                    if a == "<" {
                        Ordering::Less
                    } else if b == "<" {
                        Ordering::Greater
                    } else {
                        a.cmp(&b)
                    }
                });
                config.prefixes.dedup();
                config.prefixes.truncate(5);
            }
            Action::Remove => {
                for arg in args {
                    config.prefixes.retain(|p| p.as_str() != arg);
                    if config.prefixes.is_empty() {
                        config.prefixes.push(arg.to_owned());
                        break;
                    }
                }
            }
        }
    });
    let mut content = "Prefixes updated!\n".to_owned();
    let prefixes = ctx.config_prefixes(guild_id);
    current_prefixes(&mut content, &prefixes);
    msg.respond(&ctx, content).await?;
    Ok(())
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

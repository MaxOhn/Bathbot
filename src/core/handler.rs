use crate::{
    commands::help::{failed_help, help, help_command},
    core::{Command, CommandGroups, Context},
    BotResult, Error,
};

use std::{ops::Deref, sync::Arc};
use twilight::gateway::Event;
use uwl::Stream;

pub async fn handle_event(
    shard_id: u64,
    event: &Event,
    ctx: Arc<Context>,
    cmds: Arc<CommandGroups>,
) -> BotResult<()> {
    match &event {
        // ####################
        // ## Gateway status ##
        // ####################
        Event::ShardReconnecting(_) => info!("Shard {} is attempting to reconnect", shard_id),
        Event::ShardResuming(_) => info!("Shard {} is resuming", shard_id),
        Event::Ready(_) => info!("Shard {} ready to go!", shard_id),
        Event::Resumed => info!("Shard {} successfully resumed", shard_id),
        Event::GatewayReconnect => info!("Gateway requested shard {} to reconnect", shard_id),
        Event::GatewayInvalidateSession(recon) => {
            if *recon {
                warn!(
                    "Gateway has invalidated session for shard {}, but its reconnectable",
                    shard_id
                );
            } else {
                return Err(Error::InvalidSession(shard_id));
            }
        }
        Event::GatewayHello(u) => {
            debug!("Registered with gateway {} on shard {}", u, shard_id);
        }

        // ###########
        // ## Other ##
        // ###########
        Event::MessageCreate(msg) => {
            ctx.cache.stats.new_message(&ctx, msg);
            if msg.author.bot || msg.webhook_id.is_some() {
                return Ok(());
            }
            let prefixes = match msg.guild_id {
                Some(guild) => {
                    if !ctx.guilds.contains_key(&guild) {
                        let config = ctx.clients.psql.insert_guild(guild.0).await?;
                        ctx.guilds.insert(guild, config);
                    }
                    ctx.guilds.get(&guild).unwrap().prefixes.clone()
                }
                None => vec!["<".to_owned(), "!!".to_owned()],
            };

            let mut stream = Stream::new(&msg.content);
            stream.take_while_char(|c| c.is_whitespace());
            if !(find_prefix(&prefixes, &mut stream) || msg.guild_id.is_none()) {
                return Ok(());
            }
            stream.take_while_char(|c| c.is_whitespace());
            let invoke = parse_invoke(&mut stream, &cmds);
            match invoke {
                Invoke::Command(cmd) => ctx.cache.stats.inc_command(cmd.names[0]),
                Invoke::SubCommand { main, sub } => ctx
                    .cache
                    .stats
                    .inc_command(format!("{}-{}", main.names[0], sub.names[0])),
                Invoke::Help(_) | Invoke::FailedHelp(_) => ctx.cache.stats.inc_command("hellp"),
                _ => {}
            }
            let msg = msg.deref();
            return match invoke {
                Invoke::Command(cmd) => (cmd.fun)(&ctx, msg).await,
                Invoke::SubCommand { sub, .. } => (sub.fun)(&ctx, msg).await,
                Invoke::Help(None) => help(&ctx, &cmds, msg).await,
                Invoke::Help(Some(cmd)) => help_command(&ctx, cmd, msg).await,
                Invoke::FailedHelp(arg) => failed_help(&ctx, arg, &cmds, msg).await,
                Invoke::UnrecognisedCommand(_name) => Ok(()),
            };
        }
        _ => (),
    }
    Ok(())
}

pub fn find_prefix<'a>(prefixes: &[String], stream: &mut Stream<'a>) -> bool {
    let prefix = prefixes.iter().find_map(|p| {
        let peeked = stream.peek_for_char(p.chars().count());
        if p == peeked {
            Some(peeked)
        } else {
            None
        }
    });
    if let Some(prefix) = &prefix {
        stream.increment(prefix.chars().count());
    }
    prefix.is_some()
}

fn parse_invoke(stream: &mut Stream<'_>, groups: &CommandGroups) -> Invoke {
    let name = stream.peek_until_char(|c| c.is_whitespace()).to_lowercase();
    stream.increment(name.chars().count());
    stream.take_while_char(|c| c.is_whitespace());
    match name.as_str() {
        "h" | "help" => {
            let name = stream.peek_until_char(|c| c.is_whitespace()).to_lowercase();
            stream.increment(name.chars().count());
            stream.take_while_char(|c| c.is_whitespace());
            if name.is_empty() {
                Invoke::Help(None)
            } else if let Some(cmd) = groups.get(name.as_str()) {
                Invoke::Help(Some(cmd))
            } else {
                Invoke::FailedHelp(name)
            }
        }
        _ => {
            if let Some(cmd) = groups.get(name.as_str()) {
                let name = stream.peek_until_char(|c| c.is_whitespace()).to_lowercase();
                for sub_cmd in cmd.sub_commands {
                    if sub_cmd.names.contains(&name.as_str()) {
                        stream.increment(name.chars().count());
                        stream.take_while_char(|c| c.is_whitespace());
                        // TODO: Check permissions & co
                        // check_discrepancy(ctx, msg, config, &cmd.options)?;
                        return Invoke::SubCommand {
                            main: cmd,
                            sub: sub_cmd,
                        };
                    }
                }
                // TODO: Check permissions & co
                // check_discrepancy(ctx, msg, config, &cmd.options)?;
                Invoke::Command(cmd)
            } else {
                Invoke::UnrecognisedCommand(name)
            }
        }
    }
}

#[derive(Debug)]
pub enum Invoke {
    Command(&'static Command),
    SubCommand {
        main: &'static Command,
        sub: &'static Command,
    },
    Help(Option<&'static Command>),
    FailedHelp(String),
    UnrecognisedCommand(String),
}

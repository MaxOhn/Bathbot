mod checks;
mod parse;

use checks::{check_authority, check_ratelimit};
use parse::{find_prefix, parse_invoke, Invoke};

use crate::{
    bail,
    commands::help::{failed_help, help, help_command},
    core::{buckets::BucketName, Command, CommandGroups, Context},
    util::{constants::OWNER_USER_ID, MessageExt},
    Args, BotResult, Error,
};

use std::{
    fmt::{self, Write},
    ops::Deref,
    sync::Arc,
};
use twilight_gateway::Event;
use twilight_model::{
    channel::Message,
    gateway::presence::{ActivityType, Status},
    guild::Permissions,
};
use uwl::Stream;

pub async fn handle_event(
    shard_id: u64,
    event: &Event,
    ctx: Arc<Context>,
    cmds: Arc<CommandGroups>,
) -> BotResult<()> {
    match event {
        // ####################
        // ## Gateway status ##
        // ####################
        Event::ShardReconnecting(_) => info!("Shard {} is attempting to reconnect", shard_id),
        Event::Ready(_) => {
            let fut =
                ctx.set_shard_activity(shard_id, Status::Online, ActivityType::Playing, "osu!");
            match fut.await {
                Ok(_) => info!("Game is set for shard {}", shard_id),
                Err(why) => error!(
                    "Failed to set shard activity at ready event for shard {}: {}",
                    shard_id, why
                ),
            }
        }
        Event::GatewayReconnect => info!("Gateway requested shard {} to reconnect", shard_id),
        Event::GatewayInvalidateSession(recon) => {
            if *recon {
                warn!(
                    "Gateway has invalidated session for shard {}, but its reconnectable",
                    shard_id
                );
            } else {
                let fut = ctx.set_shard_activity(
                    shard_id,
                    Status::DoNotDisturb,
                    ActivityType::Watching,
                    "Re-gathering discord data, might take a few minutes",
                );
                match fut.await {
                    Ok(_) => info!("Updated game for shard {}'s session invalidation", shard_id),
                    Err(why) => error!(
                        "Failed to set shard activity at GatewayInvalidateSession event for shard {}: {}",
                        shard_id, why
                    ),
                }
                return Err(Error::InvalidSession(shard_id));
            }
        }
        Event::GatewayHello(u) => {
            debug!("Registered with gateway {} on shard {}", u, shard_id);
        }

        // ##############
        // ## Reaction ##
        // ##############
        Event::ReactionAdd(reaction_add) => {
            let reaction = &reaction_add.0;
            if let Some(guild_id) = reaction.guild_id {
                if let Some(role_id) = ctx.get_role_assign(reaction) {
                    match ctx.http.add_role(guild_id, reaction.user_id, role_id).await {
                        Ok(_) => debug!("Assigned react-role to user"),
                        Err(why) => error!("Error while assigning react-role to user: {}", why),
                    }
                }
            }
        }

        Event::ReactionRemove(reaction_remove) => {
            let reaction = &reaction_remove.0;
            if let Some(guild_id) = reaction.guild_id {
                if let Some(role_id) = ctx.get_role_assign(reaction) {
                    match ctx
                        .http
                        .remove_guild_member_role(guild_id, reaction.user_id, role_id)
                        .await
                    {
                        Ok(_) => debug!("Removed react-role from user"),
                        Err(why) => error!("Error while removing react-role from user: {}", why),
                    }
                }
            }
        }

        // #############
        // ## Message ##
        // #############
        Event::MessageCreate(msg) => {
            ctx.cache.stats.new_message(&ctx, msg.deref());

            // Ignore bots and webhooks
            if msg.author.bot || msg.webhook_id.is_some() {
                return Ok(());
            }

            // Get guild / default prefixes
            let prefixes = match msg.guild_id {
                Some(guild_id) => ctx.config_prefixes(guild_id),
                None => vec![String::from("<")],
            };

            // Parse msg content for prefixes
            let mut stream = Stream::new(&msg.content);
            stream.take_while_char(|c| c.is_whitespace());
            if !(find_prefix(&prefixes, &mut stream) || msg.guild_id.is_none()) {
                return Ok(());
            }

            // Parse msg content for commands
            let invoke = parse_invoke(&mut stream, &cmds);
            if let Invoke::None = invoke {
                return Ok(());
            }

            // Process invoke
            log_invoke(&ctx, msg);
            let msg = msg.deref();
            let command_result = match &invoke {
                Invoke::Command(cmd) => process_command(cmd, ctx.clone(), msg, stream).await,
                Invoke::SubCommand { sub, .. } => {
                    process_command(sub, ctx.clone(), msg, stream).await
                }
                Invoke::Help(None) => {
                    let is_authority = check_authority(&ctx, msg).transpose().is_none();
                    help(&ctx, &cmds, msg, is_authority)
                        .await
                        .map(ProcessResult::success)
                }
                Invoke::Help(Some(cmd)) => help_command(&ctx, cmd, msg)
                    .await
                    .map(ProcessResult::success),
                Invoke::FailedHelp(arg) => failed_help(&ctx, arg, &cmds, msg)
                    .await
                    .map(ProcessResult::success),
                Invoke::None => unreachable!(),
            };
            let name = invoke.name();

            // Handle processing result
            match invoke {
                Invoke::None => {}
                _ => {
                    ctx.cache.stats.inc_command(name.as_ref());
                    match command_result {
                        Ok(ProcessResult::Success) => info!("Processed command `{}`", name),
                        Ok(process_result) => {
                            info!("Command `{}` was not processed: {:?}", name, process_result)
                        }
                        Err(why) => error!("Error while processing command `{}`: {}", name, why),
                    }
                }
            }
        }
        _ => (),
    }
    Ok(())
}

fn log_invoke(ctx: &Context, msg: &Message) {
    let mut location = String::with_capacity(31);
    match msg.guild_id.and_then(|id| ctx.cache.get_guild(id)) {
        Some(guild) => {
            let _ = write!(location, "{}", guild.name);
            location.push(':');
            match ctx.cache.guild_channels.get(&msg.channel_id) {
                Some(guard) => location.push_str(guard.value().get_name()),
                None => location.push_str("<uncached channel>"),
            }
        }
        None => location.push_str("Private"),
    }
    info!("[{}] {}: {}", location, msg.author.name, msg.content);
}

#[derive(Debug)]
enum ProcessResult {
    Success,
    NoDM,
    NoSendPermission,
    Ratelimited(BucketName),
    NoOwner,
    NoAuthority,
}

impl ProcessResult {
    fn success(_: ()) -> Self {
        Self::Success
    }
}

impl fmt::Display for ProcessResult {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ratelimited(bucket) => write!(fmt, "Ratelimited ({:?})", bucket),
            _ => write!(fmt, "{:?}", self),
        }
    }
}

async fn process_command(
    cmd: &Command,
    ctx: Arc<Context>,
    msg: &Message,
    stream: Stream<'_>,
) -> BotResult<ProcessResult> {
    // Only in guilds?
    if (cmd.authority || cmd.only_guilds) && msg.guild_id.is_none() {
        let content = "That command is only available in guilds";
        msg.error(&ctx, content).await?;
        return Ok(ProcessResult::NoDM);
    }

    // Only for owner?
    if cmd.owner && msg.author.id.0 != OWNER_USER_ID {
        let content = "That command can only be used by the bot owner";
        msg.error(&ctx, content).await?;
        return Ok(ProcessResult::NoOwner);
    }

    // Does bot have sufficient permissions to send response?
    let permissions =
        ctx.cache
            .get_channel_permissions_for(ctx.cache.bot_user.id, msg.channel_id, msg.guild_id);
    if !permissions.contains(Permissions::SEND_MESSAGES) {
        debug!("No SEND_MESSAGE permission, can not respond");
        return Ok(ProcessResult::NoSendPermission);
    }

    // Ratelimited?
    if let Some(bucket) = cmd.bucket {
        if let Some(cooldown) = check_ratelimit(&ctx, msg, bucket).await {
            debug!(
                "Ratelimiting user {} on command `{}` for {} seconds",
                msg.author.id, cmd.names[0], cooldown,
            );
            let content = format!("Command on cooldown, try again in {} seconds", cooldown);
            msg.error(&ctx, content).await?;
            return Ok(ProcessResult::Ratelimited(bucket.into()));
        }
    }

    // Only for authorities?
    if cmd.authority {
        match check_authority(&ctx, msg) {
            Ok(None) => {}
            Ok(Some(content)) => {
                debug!(
                    "Non-authority user {} tried using command `{}`",
                    msg.author.id, cmd.names[0]
                );
                msg.error(&ctx, content).await?;
                return Ok(ProcessResult::NoAuthority);
            }
            Err(why) => {
                let content = "Error while checking authority status";
                let _ = msg.error(&ctx, content).await;
                bail!("error while checking authorty status: {}", why);
            }
        }
    }

    // Prepare lightweight arguments
    let args = Args::new(&msg.content, stream);

    // Broadcast typing event
    let _ = ctx.http.create_typing_trigger(msg.channel_id).await;

    // Call command function
    (cmd.fun)(ctx, msg, args).await.map(ProcessResult::success)
}

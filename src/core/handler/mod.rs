mod checks;
mod parse;

use checks::{check_authority, check_ratelimit};
use parse::{find_prefix, parse_invoke, Invoke};

use crate::{
    arguments::Stream,
    commands::help::{failed_help, help, help_command},
    core::{buckets::BucketName, Command, Context},
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

pub async fn handle_event(shard_id: u64, event: Event, ctx: Arc<Context>) -> BotResult<()> {
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
            if recon {
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

        // ##############
        // ## Reaction ##
        // ##############
        Event::ReactionAdd(reaction_add) => {
            let reaction = &reaction_add.0;
            if let Some(guild_id) = reaction.guild_id {
                if let Some(role_id) = ctx.get_role_assign(reaction) {
                    match ctx
                        .http
                        .add_guild_member_role(guild_id, reaction.user_id, role_id)
                        .exec()
                        .await
                    {
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
                        .exec()
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
        Event::MessageDelete(msg) => {
            ctx.remove_msg(msg.id);
        }
        Event::MessageDeleteBulk(msgs) => msgs.ids.into_iter().for_each(|id| {
            ctx.remove_msg(id);
        }),
        Event::MessageCreate(msg) => {
            ctx.stats.new_message(&ctx, msg.deref());

            // Ignore bots and webhooks
            if msg.author.bot || msg.webhook_id.is_some() {
                return Ok(());
            }

            // Get guild / default prefixes
            let prefixes = match msg.guild_id {
                Some(guild_id) => ctx.config_prefixes(guild_id),
                None => smallvec!["<".into()],
            };

            // Parse msg content for prefixes
            let mut stream = Stream::new(&msg.content);
            stream.take_while_char(|c| c.is_whitespace());

            if !(find_prefix(&prefixes, &mut stream) || msg.guild_id.is_none()) {
                return Ok(());
            }

            // Parse msg content for commands
            let invoke = parse_invoke(&mut stream);

            if let Invoke::None = invoke {
                return Ok(());
            }

            // Process invoke
            log_invoke(&ctx, &msg);
            let msg = msg.deref();

            let command_result = match &invoke {
                Invoke::Command { cmd, num } => {
                    process_command(cmd, Arc::clone(&ctx), msg, stream, *num).await
                }
                Invoke::SubCommand { sub, .. } => {
                    process_command(sub, Arc::clone(&ctx), msg, stream, None).await
                }
                Invoke::Help(None) => {
                    let is_authority = check_authority(&ctx, msg).transpose().is_none();

                    help(&ctx, msg, is_authority)
                        .await
                        .map(ProcessResult::success)
                }
                Invoke::Help(Some(cmd)) => help_command(&ctx, cmd, msg)
                    .await
                    .map(ProcessResult::success),
                Invoke::FailedHelp(arg) => failed_help(&ctx, arg, msg)
                    .await
                    .map(ProcessResult::success),
                Invoke::None => unreachable!(),
            };

            let name = invoke.name();

            // Handle processing result
            match invoke {
                Invoke::None => {}
                _ => {
                    ctx.stats.inc_command(name.as_ref());

                    match command_result {
                        Ok(ProcessResult::Success) => info!("Processed command `{}`", name),
                        Ok(process_result) => {
                            info!("Command `{}` was not processed: {:?}", name, process_result)
                        }
                        Err(why) => return Err(Error::Command(Box::new(why), name.into_owned())),
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

    match msg.guild_id.and_then(|id| ctx.cache.guild(id)) {
        Some(guild) => {
            let _ = write!(location, "{}", guild.name);
            location.push(':');

            match ctx.cache.guild_channel(msg.channel_id) {
                Some(channel) => location.push_str(channel.name()),
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
    #[inline]
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
    num: Option<usize>,
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
    match ctx.cache.current_user() {
        Some(bot_user) => {
            let permissions =
                ctx.cache
                    .get_channel_permissions_for(bot_user.id, msg.channel_id, msg.guild_id);

            if !permissions.contains(Permissions::SEND_MESSAGES) {
                debug!("No SEND_MESSAGE permission, can not respond");

                return Ok(ProcessResult::NoSendPermission);
            }
        }
        None => error!("No CurrentUser in cache for permission check"),
    };

    // Ratelimited?
    {
        let guard = ctx.buckets.get(&BucketName::All).unwrap();
        let mutex = guard.value();
        let mut bucket = mutex.lock().await;
        let ratelimit = bucket.take(msg.author.id.0);

        if ratelimit > 0 {
            debug!(
                "Ratelimiting user {} for {} seconds",
                msg.author.id, ratelimit,
            );

            return Ok(ProcessResult::Ratelimited(BucketName::All));
        }
    }

    if let Some(bucket) = cmd.bucket {
        if let Some((cooldown, bucket)) = check_ratelimit(&ctx, msg, bucket).await {
            debug!(
                "Ratelimiting user {} on command `{}` for {} seconds",
                msg.author.id, cmd.names[0], cooldown,
            );

            if !matches!(bucket, BucketName::BgHint) {
                let content = format!("Command on cooldown, try again in {} seconds", cooldown);
                msg.error(&ctx, content).await?;
            }

            return Ok(ProcessResult::Ratelimited(bucket));
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

                return Err(Error::Authority(Box::new(why)));
            }
        }
    }

    // Prepare lightweight arguments
    let args = Args::new(&msg.content, stream);

    // Broadcast typing event
    let _ = ctx.http.create_typing_trigger(msg.channel_id).exec().await;

    // Call command function
    (cmd.fun)(ctx, msg, args, num).await?;

    Ok(ProcessResult::Success)
}

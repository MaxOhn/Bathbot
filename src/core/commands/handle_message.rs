use super::{parse, Command, Invoke, ProcessResult};
use crate::{
    arguments::{Args, Stream},
    commands::help::{failed_help, help, help_command},
    core::buckets::BucketName,
    util::{constants::OWNER_USER_ID, MessageExt},
    BotResult, CommandData, Context, Error,
};

use std::sync::Arc;
use twilight_model::{channel::Message, guild::Permissions};

pub async fn handle_message(ctx: Arc<Context>, msg: Message) -> BotResult<()> {
    // Ignore bots and webhooks
    if msg.author.bot || msg.webhook_id.is_some() {
        return Ok(());
    }

    // Get guild / default prefixes
    let prefixes = match msg.guild_id {
        Some(guild_id) => ctx.config_prefixes(guild_id).await,
        None => smallvec!["<".into()],
    };

    // Parse msg content for prefixes
    let mut stream = Stream::new(&msg.content);
    stream.take_while_char(|c| c.is_whitespace());

    if !(parse::find_prefix(&prefixes, &mut stream) || msg.guild_id.is_none()) {
        return Ok(());
    }

    // Parse msg content for commands
    let invoke = parse::parse_invoke(&mut stream);

    if let Invoke::None = invoke {
        return Ok(());
    }

    // Process invoke
    log_invoke(&ctx, &msg);
    let name = invoke.name();
    ctx.stats.increment_message_command(name.as_ref());

    let command_result = match &invoke {
        Invoke::Command { cmd, num } => process_command(cmd, ctx, &msg, stream, *num).await,
        Invoke::SubCommand { sub, .. } => process_command(sub, ctx, &msg, stream, None).await,
        Invoke::Help(None) => {
            let is_authority = super::check_authority(&ctx, &msg)
                .await
                .transpose()
                .is_none();

            let args = Args::new(&msg.content, stream);

            let data = CommandData::Message {
                msg: &msg,
                args,
                num: None,
            };

            help(&ctx, data, is_authority)
                .await
                .map(ProcessResult::success)
        }
        Invoke::Help(Some(cmd)) => help_command(&ctx, cmd, msg.guild_id, (&msg).into())
            .await
            .map(ProcessResult::success),
        Invoke::FailedHelp(arg) => failed_help(&ctx, arg, (&msg).into())
            .await
            .map(ProcessResult::success),
        Invoke::None => unreachable!(),
    };

    // Handle processing result
    match command_result {
        Ok(ProcessResult::Success) => info!("Processed command `{}`", name),
        Ok(result) => info!("Command `{}` was not processed: {:?}", name, result),
        Err(why) => return Err(Error::Command(Box::new(why), name.into_owned())),
    }

    Ok(())
}

fn log_invoke(ctx: &Context, msg: &Message) {
    let mut location = String::with_capacity(31);

    match msg.guild_id.and_then(|id| ctx.cache.guild(id)) {
        Some(guild) => {
            location.push_str(guild.name());
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
    if cmd.owner && msg.author.id.get() != OWNER_USER_ID {
        let content = "That command can only be used by the bot owner";
        msg.error(&ctx, content).await?;

        return Ok(ProcessResult::NoOwner);
    }

    // Does bot have sufficient permissions to send response in a guild?
    if msg.guild_id.is_some() {
        match ctx.cache.current_user() {
            Some(bot_user) => {
                let permissions = ctx
                    .cache
                    .permissions()
                    .in_channel(bot_user.id, msg.channel_id)
                    .ok()
                    .unwrap_or_else(Permissions::empty);

                if !permissions.contains(Permissions::SEND_MESSAGES) {
                    debug!("No SEND_MESSAGE permission, can not respond");

                    return Ok(ProcessResult::NoSendPermission);
                }
            }
            None => error!("No CurrentUser in cache for permission check"),
        };
    }

    // Ratelimited?
    {
        let guard = ctx.buckets.get(&BucketName::All).unwrap();
        let mutex = guard.value();
        let mut bucket = mutex.lock();
        let ratelimit = bucket.take(msg.author.id.get());

        if ratelimit > 0 {
            debug!(
                "Ratelimiting user {} for {} seconds",
                msg.author.id, ratelimit,
            );

            return Ok(ProcessResult::Ratelimited(BucketName::All));
        }
    }

    if let Some(bucket) = cmd.bucket {
        if let Some((cooldown, bucket)) = super::check_ratelimit(&ctx, msg, bucket).await {
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
        match super::check_authority(&ctx, msg).await {
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
    if cmd.typing {
        let _ = ctx.http.create_typing_trigger(msg.channel_id).exec().await;
    }

    // Call command function
    (cmd.fun)(ctx, msg, args, num).await?;

    Ok(ProcessResult::Success)
}

use super::{parse, Command, Invoke, ProcessResult, RetrievedCacheData};
use crate::{
    arguments::{Args, Stream},
    commands::help::{failed_help, help, help_command},
    core::buckets::BucketName,
    util::{constants::OWNER_USER_ID, MessageExt},
    BotResult, CommandData, Context, Error,
};

use bathbot_cache::model::{ChannelOrId, GuildOrId};
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
    let cache_data = log_invoke(&ctx, &msg).await;
    let name = invoke.name();
    ctx.stats.increment_message_command(name.as_ref());

    let command_result = match &invoke {
        Invoke::Command { cmd, num } => {
            process_command(cmd, ctx, &msg, stream, *num, cache_data).await
        }
        Invoke::SubCommand { sub, .. } => {
            process_command(sub, ctx, &msg, stream, None, cache_data).await
        }
        Invoke::Help(None) => {
            let is_authority = super::check_authority(&ctx, &msg, cache_data.guild.as_ref())
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

async fn log_invoke(ctx: &Context, msg: &Message) -> RetrievedCacheData {
    let mut location = String::with_capacity(31);

    let guild = match msg.guild_id {
        Some(guild) => ctx.cache.guild(guild).await.ok().flatten(),
        None => None,
    };

    let channel = match guild {
        Some(ref guild) => {
            location.push_str(guild.name.as_str());
            location.push(':');

            match ctx.cache.channel(msg.channel_id).await {
                Ok(Some(channel)) => {
                    location.push_str(channel.name());

                    Some(channel)
                }
                _ => {
                    location.push_str("<uncached channel>");

                    None
                }
            }
        }
        None => {
            location.push_str("Private");
            None
        }
    };

    info!("[{}] {}: {}", location, msg.author.name, msg.content);

    let guild = guild.map(GuildOrId::Guild);
    let channel = channel.map(ChannelOrId::Channel);

    RetrievedCacheData { guild, channel }
}

async fn process_command(
    cmd: &Command,
    ctx: Arc<Context>,
    msg: &Message,
    stream: Stream<'_>,
    num: Option<usize>,
    cache_data: RetrievedCacheData,
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

    let guild = match cache_data.guild {
        Some(guild) => Some(guild),
        None => msg.guild_id.map(From::from),
    };

    let channel = cache_data.channel.unwrap_or_else(|| msg.channel_id.into());

    // Does bot have sufficient permissions to send response in a guild?
    if msg.guild_id.is_some() {
        let user_id = ctx.current_user.read().id;

        let permissions = ctx
            .cache
            .get_channel_permissions(user_id, &channel, guild.as_ref())
            .await?;

        if !permissions.contains(Permissions::SEND_MESSAGES) {
            return Ok(ProcessResult::NoSendPermission);
        }
    }

    // Ratelimited?
    {
        let guard = ctx.buckets.get(&BucketName::All).unwrap();
        let mutex = guard.value();
        let mut bucket = mutex.lock();
        let ratelimit = bucket.take(msg.author.id.get());

        if ratelimit > 0 {
            trace!(
                "Ratelimiting user {} for {} seconds",
                msg.author.id,
                ratelimit,
            );

            return Ok(ProcessResult::Ratelimited(BucketName::All));
        }
    }

    if let Some(bucket) = cmd.bucket {
        if let Some((cooldown, bucket)) = super::check_ratelimit(&ctx, msg, bucket).await {
            trace!(
                "Ratelimiting user {} on command `{}` for {} seconds",
                msg.author.id,
                cmd.names[0],
                cooldown,
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
        match super::check_authority(&ctx, msg, guild.as_ref()).await {
            Ok(None) => {}
            Ok(Some(content)) => {
                let _ = msg.error(&ctx, content).await;

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

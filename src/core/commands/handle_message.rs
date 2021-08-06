use super::{parse, Command, Invoke};
use crate::{
    arguments::{Args, Stream},
    bail,
    commands::help::{failed_help, help, help_command},
    core::buckets::BucketName,
    util::{constants::OWNER_USER_ID, MessageExt},
    BotResult, Context, Error,
};

use std::{
    fmt::{self, Write},
    sync::Arc,
};
use twilight_model::{channel::Message, guild::Permissions, id::RoleId};

pub async fn handle_message(ctx: Arc<Context>, msg: Message) -> BotResult<()> {
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

    let command_result = match &invoke {
        Invoke::Command { cmd, num } => {
            process_command(cmd, Arc::clone(&ctx), &msg, stream, *num).await
        }
        Invoke::SubCommand { sub, .. } => {
            process_command(sub, Arc::clone(&ctx), &msg, stream, None).await
        }
        Invoke::Help(None) => {
            let is_authority = check_authority(&ctx, &msg).transpose().is_none();

            help(&ctx, &msg, is_authority)
                .await
                .map(ProcessResult::success)
        }
        Invoke::Help(Some(cmd)) => help_command(&ctx, cmd, &msg)
            .await
            .map(ProcessResult::success),
        Invoke::FailedHelp(arg) => failed_help(&ctx, arg, &msg)
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

    Ok(())
}

fn log_invoke(ctx: &Context, msg: &Message) {
    let mut location = String::with_capacity(31);

    match msg.guild_id.and_then(|id| ctx.cache.guild(id)) {
        Some(guild) => {
            location.push_str(guild.name.as_str());
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

// Is authority -> Ok(None)
// No authority -> Ok(Some(message to user))
// Couldn't figure out -> Err()
pub fn check_authority(ctx: &Context, msg: &Message) -> BotResult<Option<String>> {
    let guild_id = match msg.guild_id {
        Some(id) => id,
        None => return Ok(Some(String::new())),
    };

    let permissions = ctx
        .cache
        .get_guild_permissions_for(msg.author.id, msg.guild_id);

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(None);
    }

    let auth_roles = ctx.config_authorities_collect(guild_id, RoleId);

    if auth_roles.is_empty() {
        let prefix = ctx.config_first_prefix(Some(guild_id));

        let content = format!(
            "You need admin permissions to use this command.\n\
            (`{}help authorities` to adjust authority status for this server)",
            prefix
        );

        return Ok(Some(content));
    } else if let Some(member) = ctx.cache.member(guild_id, msg.author.id) {
        if !member.roles.iter().any(|role| auth_roles.contains(role)) {
            let roles: Vec<_> = auth_roles
                .iter()
                .filter_map(|&role| {
                    ctx.cache.role(role).map_or_else(
                        || {
                            warn!("Role {} not cached for guild {}", role, guild_id);

                            None
                        },
                        |role| Some(role.name),
                    )
                })
                .collect();

            let role_len: usize = roles.iter().map(|role| role.len()).sum();

            let mut content = String::from(
                "You need either admin permissions or \
                any of these roles to use this command:\n",
            );

            content.reserve_exact(role_len + roles.len().saturating_sub(1) * 4);
            let mut roles = roles.into_iter();

            if let Some(first) = roles.next() {
                content.push_str(&first);

                for role in roles {
                    let _ = write!(content, ", `{}`", role);
                }
            }

            let prefix = ctx.config_first_prefix(Some(guild_id));

            let _ = write!(
                content,
                "\n(`{}help authorities` to adjust authority status for this server)",
                prefix
            );

            return Ok(Some(content));
        }
    } else {
        bail!("member {} not cached for guild {}", msg.author.id, guild_id);
    }

    Ok(None)
}

pub async fn check_ratelimit(
    ctx: &Context,
    msg: &Message,
    bucket: impl AsRef<str>,
) -> Option<(i64, BucketName)> {
    let (ratelimit, bucket) = {
        let bucket: BucketName = bucket.as_ref().parse().unwrap();
        let guard = ctx.buckets.get(&bucket).unwrap();
        let mutex = guard.value();
        let mut bucket_elem = mutex.lock().await;

        match bucket {
            BucketName::Snipe => (bucket_elem.take(0), bucket), // same bucket for everyone
            BucketName::Songs => (
                bucket_elem.take(
                    msg.guild_id
                        .map_or_else(|| msg.author.id.0, |guild_id| guild_id.0), // same bucket for guilds
                ),
                bucket,
            ),
            _ => (bucket_elem.take(msg.author.id.0), bucket),
        }
    };

    if ratelimit > 0 {
        return Some((ratelimit, bucket));
    }

    None
}

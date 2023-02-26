use std::sync::Arc;

use bathbot_psql::model::configs::{GuildConfig, Prefix, DEFAULT_PREFIX};
use eyre::Result;
use nom::{
    bytes::complete as by,
    combinator::{opt, recognize},
};
use twilight_model::{channel::Message, guild::Permissions};

use crate::{
    core::{
        buckets::BucketName,
        commands::checks::{check_authority, check_ratelimit},
        Context,
    },
    util::ChannelExt,
};

use self::parse::*;

use super::{EventKind, ProcessResult};

mod parse;

pub async fn handle_message(ctx: Arc<Context>, msg: Message) {
    // Ignore bots and webhooks
    if msg.author.bot || msg.webhook_id.is_some() {
        return;
    }

    let content = msg.content.as_str();

    // Check msg content for a prefix
    let prefix_opt = if let Some(guild_id) = msg.guild_id {
        let f = |config: &GuildConfig| {
            config
                .prefixes
                .iter()
                .map(Prefix::as_str)
                .map(|p| by::tag::<_, _, ()>(p)(content))
                .flat_map(Result::ok)
                .max_by_key(|(_, p)| p.len())
        };

        ctx.guild_config().peek(guild_id, f).await
    } else {
        recognize::<_, _, (), _>(opt(by::tag(DEFAULT_PREFIX)))(content).ok()
    };

    let Some((content, _)) = prefix_opt else {
        return;
    };

    // Parse msg content for commands
    let Some(invoke) = Invoke::parse(content) else {
        return;
    };

    let name = invoke.cmd.name();
    EventKind::PrefixCommand.log(&ctx, &msg, name);
    ctx.stats.increment_message_command(name);

    match process_command(ctx, invoke, &msg).await {
        Ok(ProcessResult::Success) => info!("Processed command `{name}`"),
        Ok(result) => info!("Command `{name}` was not processed: {result:?}"),
        Err(err) => {
            let wrap = format!("Failed to process prefix command `{name}`");
            error!("{:?}", err.wrap_err(wrap));
        }
    }
}

async fn process_command<'m>(
    ctx: Arc<Context>,
    invoke: Invoke<'m>,
    msg: &'m Message,
) -> Result<ProcessResult> {
    let Invoke { cmd, args } = invoke;

    // Only in guilds?
    if (cmd.flags.authority() || cmd.flags.only_guilds()) && msg.guild_id.is_none() {
        let content = "That command is only available in servers";
        msg.error(&ctx, content).await?;

        return Ok(ProcessResult::NoDM);
    }

    // Only for owner?
    // * Not necessary since there are no owner-only prefix commands

    let channel = msg.channel_id;

    // Does bot have sufficient permissions to send response in a guild?
    let permissions = if let Some(guild) = msg.guild_id {
        let user = ctx.cache.current_user(|user| user.id)?;
        let permissions = ctx.cache.get_channel_permissions(user, channel, guild);

        if !permissions.contains(Permissions::SEND_MESSAGES) {
            return Ok(ProcessResult::NoSendPermission);
        }

        Some(permissions)
    } else {
        None
    };

    // Ratelimited?
    let ratelimit = ctx
        .buckets
        .get(BucketName::All)
        .lock()
        .take(msg.author.id.get());

    if ratelimit > 0 {
        trace!(
            "Ratelimiting user {} for {ratelimit} seconds",
            msg.author.id,
        );

        return Ok(ProcessResult::Ratelimited(BucketName::All));
    }

    if let Some(bucket) = cmd.bucket {
        if let Some(cooldown) = check_ratelimit(&ctx, msg.author.id, bucket).await {
            trace!(
                "Ratelimiting user {} on bucket `{bucket:?}` for {cooldown} seconds",
                msg.author.id,
            );

            let content = format!("Command on cooldown, try again in {cooldown} seconds");
            msg.error(&ctx, content).await?;

            return Ok(ProcessResult::Ratelimited(bucket));
        }
    }

    // Only for authorities?
    if cmd.flags.authority() {
        match check_authority(&ctx, msg.author.id, msg.guild_id).await {
            Ok(None) => {}
            Ok(Some(content)) => {
                let _ = msg.error(&ctx, content).await;

                return Ok(ProcessResult::NoAuthority);
            }
            Err(err) => {
                let content = "Error while checking authority status";
                let _ = msg.error(&ctx, content).await;

                return Err(err.wrap_err("failed to check authority status"));
            }
        }
    }

    // Broadcast typing event
    if cmd.flags.defer() {
        let _ = ctx.http.create_typing_trigger(channel).await;
    }

    // Call command function
    (cmd.exec)(ctx, msg, args, permissions).await?;

    Ok(ProcessResult::Success)
}

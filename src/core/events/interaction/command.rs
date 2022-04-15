use std::{mem, sync::Arc};

use eyre::Report;
use twilight_model::{application::interaction::ApplicationCommand, guild::Permissions};

use crate::{
    core::{
        commands::{
            checks::{check_authority, check_ratelimit},
            slash::{SlashCommand, SLASH_COMMANDS},
        },
        events::{log_command, ProcessResult},
        Context, CONFIG,
    },
    error::Error,
    util::{ApplicationCommandExt, Authored},
    BotResult,
};

pub async fn handle_command(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) {
    let name = mem::take(&mut command.data.name);
    log_command(&ctx, &*command, &name);
    ctx.stats.increment_slash_command(&name);

    let slash = match SLASH_COMMANDS.command(&name) {
        Some(slash) => slash,
        None => return error!("unknown slash command `{name}`"),
    };

    match process_command(ctx, command, slash).await {
        Ok(ProcessResult::Success) => info!("Processed slash command `{name}`"),
        Ok(res) => info!("Command `/{name}` was not processed: {res:?}"),
        Err(err) => {
            let wrap = format!("failed to process slash command `{name}`");
            error!("{:?}", Report::new(err).wrap_err(wrap));
        }
    }
}

async fn process_command(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    slash: &SlashCommand,
) -> BotResult<ProcessResult> {
    match pre_process_command(&ctx, &command, slash).await? {
        Some(result) => Ok(result),
        None => {
            if slash.flags.defer() {
                command.defer(&ctx, slash.flags.ephemeral()).await?;
            }

            (slash.exec)(ctx, command).await?;

            Ok(ProcessResult::Success)
        }
    }
}

async fn pre_process_command(
    ctx: &Context,
    command: &ApplicationCommand,
    slash: &SlashCommand,
) -> BotResult<Option<ProcessResult>> {
    let guild_id = command.guild_id;

    // Only in guilds?
    if slash.flags.only_guilds() && guild_id.is_none() {
        let content = "That command is only available in servers";
        command.error_callback(ctx, content).await?;

        return Ok(Some(ProcessResult::NoDM));
    }

    let user_id = command.user_id()?;

    // Only for owner?
    if slash.flags.only_owner() && user_id != CONFIG.get().unwrap().owner {
        let content = "That command can only be used by the bot owner";
        command.error_callback(ctx, content).await?;

        return Ok(Some(ProcessResult::NoOwner));
    }

    // Does bot have sufficient permissions to send response in a guild?
    // Technically not necessary but there is currently no other way for
    // users to disable slash commands in certain channels.
    if let Some(guild) = command.guild_id {
        let user = ctx.cache.current_user()?.id;
        let channel = command.channel_id;
        let permissions = ctx.cache.get_channel_permissions(user, channel, guild);

        if !permissions.contains(Permissions::SEND_MESSAGES) {
            let content = "I have no send permission in this channel so I won't process commands";
            command.error_callback(ctx, content).await?;

            return Ok(Some(ProcessResult::NoSendPermission));
        }
    }

    // Ratelimited?
    if let Some(bucket) = slash.bucket {
        if let Some(cooldown) = check_ratelimit(ctx, user_id, bucket).await {
            trace!("Ratelimiting user {user_id} on bucket `{bucket:?}` for {cooldown} seconds");

            let content = format!("Command on cooldown, try again in {cooldown} seconds");
            command.error_callback(ctx, content).await?;

            return Ok(Some(ProcessResult::Ratelimited(bucket)));
        }
    }

    // Only for authorities?
    if slash.flags.authority() {
        match check_authority(ctx, user_id, command.guild_id).await {
            Ok(None) => {}
            Ok(Some(content)) => {
                command.error_callback(ctx, content).await?;

                return Ok(Some(ProcessResult::NoAuthority));
            }
            Err(err) => {
                let content = "Error while checking authority status";
                let _ = command.error_callback(ctx, content).await;

                return Err(Error::Authority(Box::new(err)));
            }
        }
    }

    Ok(None)
}

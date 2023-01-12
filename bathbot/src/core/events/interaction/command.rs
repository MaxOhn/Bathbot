use std::{mem, sync::Arc};

use eyre::Result;

use crate::{
    core::{
        commands::{
            checks::{check_authority, check_ratelimit},
            slash::{SlashCommand, SlashCommands},
        },
        events::{EventKind, ProcessResult},
        BotConfig, Context,
    },
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

pub async fn handle_command(ctx: Arc<Context>, mut command: InteractionCommand) {
    let name = mem::take(&mut command.data.name);
    EventKind::SlashCommand.log(&ctx, &command, &name);
    ctx.stats.increment_slash_command(&name);

    let slash = match SlashCommands::get().command(&name) {
        Some(slash) => slash,
        None => return error!("unknown slash command `{name}`"),
    };

    match process_command(ctx, command, slash).await {
        Ok(ProcessResult::Success) => info!("Processed slash command `{name}`"),
        Ok(res) => info!("Command `/{name}` was not processed: {res:?}"),
        Err(err) => {
            let wrap = format!("Failed to process slash command `{name}`");
            error!("{:?}", err.wrap_err(wrap));
        }
    }
}

async fn process_command(
    ctx: Arc<Context>,
    command: InteractionCommand,
    slash: &SlashCommand,
) -> Result<ProcessResult> {
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
    command: &InteractionCommand,
    slash: &SlashCommand,
) -> Result<Option<ProcessResult>> {
    let user_id = command.user_id()?;

    // Only for owner?
    if slash.flags.only_owner() && user_id != BotConfig::get().owner {
        let content = "That command can only be used by the bot owner";
        command.error_callback(ctx, content).await?;

        return Ok(Some(ProcessResult::NoOwner));
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

                return Err(err.wrap_err("failed to check authority status"));
            }
        }
    }

    Ok(None)
}

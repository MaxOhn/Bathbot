use std::{mem, sync::Arc};

use eyre::Result;

use crate::{
    core::{
        commands::{
            checks::{check_authority, check_ratelimit},
            interaction::{InteractionCommandKind, InteractionCommands, SlashCommand},
        },
        events::{EventKind, ProcessResult},
        BotConfig, Context,
    },
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

pub async fn handle_command(ctx: Arc<Context>, mut command: InteractionCommand) {
    let name = mem::take(&mut command.data.name);
    EventKind::InteractionCommand
        .log(&ctx, &command, &name)
        .await;

    let Some(cmd) = InteractionCommands::get().command(&name) else {
        return error!(name, "Unknown interaction command");
    };

    match process_command(ctx, command, cmd).await {
        Ok(ProcessResult::Success) => info!(%name, "Processed interaction command"),
        Ok(reason) => info!(?reason, "Interaction command `{name}` was not processed"),
        Err(err) => error!(name, ?err, "Failed to process interaction command"),
    }
}

async fn process_command(
    ctx: Arc<Context>,
    command: InteractionCommand,
    cmd: InteractionCommandKind,
) -> Result<ProcessResult> {
    match cmd {
        InteractionCommandKind::Chat(cmd) => {
            match pre_process_command(&ctx, &command, cmd).await? {
                Some(result) => return Ok(result),
                None => {
                    if cmd.flags.defer() {
                        command.defer(&ctx, cmd.flags.ephemeral()).await?;
                    }

                    (cmd.exec)(ctx, command).await?;
                }
            }
        }
        InteractionCommandKind::Message(cmd) => {
            if cmd.flags.defer() {
                command.defer(&ctx, cmd.flags.ephemeral()).await?;
            }

            (cmd.exec)(ctx, command).await?;
        }
    }

    Ok(ProcessResult::Success)
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

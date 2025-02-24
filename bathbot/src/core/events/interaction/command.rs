use std::{mem, time::Instant};

use eyre::Result;

use crate::{
    core::{
        BotConfig, BotMetrics, Context,
        commands::{
            checks::check_authority,
            interaction::{InteractionCommandKind, InteractionCommands, SlashCommand},
        },
        events::{EventKind, ProcessResult},
    },
    util::{Authored, InteractionCommandExt, interaction::InteractionCommand},
};

pub async fn handle_command(mut command: InteractionCommand) {
    let start = Instant::now();

    let name = mem::take(&mut command.data.name);
    EventKind::InteractionCommand.log(&command, &name).await;

    let Some(cmd) = InteractionCommands::get().command(&name) else {
        return error!(name, "Unknown interaction command");
    };

    let group_sub = command.group_sub();

    match process_command(command, cmd).await {
        Ok(ProcessResult::Success) => info!(%name, "Processed interaction command"),
        Ok(reason) => info!(?reason, "Interaction command `{name}` was not processed"),
        Err(err) => {
            match group_sub.clone() {
                Some((group, sub)) => BotMetrics::inc_slash_command_error(name.clone(), group, sub),
                None => BotMetrics::inc_command_error("message", name.clone()),
            }

            error!(name, ?err, "Failed to process interaction command");
        }
    }

    let elapsed = start.elapsed();

    match group_sub {
        Some((group, sub)) => BotMetrics::observe_slash_command(name, group, sub, elapsed),
        None => BotMetrics::observe_command("message", name, elapsed),
    }
}

async fn process_command(
    command: InteractionCommand,
    cmd: InteractionCommandKind,
) -> Result<ProcessResult> {
    match cmd {
        InteractionCommandKind::Chat(cmd) => match pre_process_command(&command, cmd).await? {
            Some(result) => return Ok(result),
            None => {
                if cmd.flags.defer() {
                    command.defer(cmd.flags.ephemeral()).await?;
                }

                (cmd.exec)(command).await?;
            }
        },
        InteractionCommandKind::Message(cmd) => {
            if cmd.flags.defer() {
                command.defer(cmd.flags.ephemeral()).await?;
            }

            (cmd.exec)(command).await?;
        }
    }

    Ok(ProcessResult::Success)
}

async fn pre_process_command(
    command: &InteractionCommand,
    slash: &SlashCommand,
) -> Result<Option<ProcessResult>> {
    let user_id = command.user_id()?;

    // Only for owner?
    if slash.flags.only_owner() && user_id != BotConfig::get().owner {
        let content = "That command can only be used by the bot owner";
        command.error_callback(content).await?;

        return Ok(Some(ProcessResult::NoOwner));
    }

    // Only in guilds?
    // Using `dm_permission = false` used to be sufficient but apparently
    // that's no longer the case.
    if slash.flags.only_guilds() && command.guild_id.is_none() {
        let content = "That command is only available in servers";
        command.error_callback(content).await?;

        return Ok(Some(ProcessResult::NoDM));
    }

    // Ratelimited?
    if let Some(bucket) = slash.bucket {
        if let Some(cooldown) = Context::check_ratelimit(user_id, bucket) {
            trace!("Ratelimiting user {user_id} on bucket `{bucket:?}` for {cooldown} seconds");

            let content = format!("Command on cooldown, try again in {cooldown} seconds");
            command.error_callback(content).await?;

            return Ok(Some(ProcessResult::Ratelimited(bucket)));
        }
    }

    // Only for authorities?
    if slash.flags.authority() {
        match check_authority(user_id, command.guild_id).await {
            Ok(None) => {}
            Ok(Some(content)) => {
                command.error_callback(content).await?;

                return Ok(Some(ProcessResult::NoAuthority));
            }
            Err(err) => {
                let content = "Error while checking authority status";
                let _ = command.error_callback(content).await;

                return Err(err.wrap_err("failed to check authority status"));
            }
        }
    }

    Ok(None)
}

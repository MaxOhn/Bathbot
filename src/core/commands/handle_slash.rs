use super::ProcessResult;
use crate::{
    commands::{fun, help, osu, owner, songs, tracking, twitch, utility},
    core::buckets::BucketName,
    util::{constants::OWNER_USER_ID, ApplicationCommandExt, Authored, MessageExt},
    BotResult, Context, Error,
};

use std::{future::Future, sync::Arc};
use twilight_model::{application::interaction::ApplicationCommand, guild::Permissions};

#[derive(Default)]
struct CommandArgs {
    only_owner: bool,
    authority: bool,
    only_guilds: bool,
    bucket: Option<BucketName>,
}

pub async fn handle_interaction(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    // TODO: Command count metric
    // TODO: Extend 3s response time for long commands

    let cmd_name = command.data.name.to_owned();
    log_slash(&ctx, &command, cmd_name.as_str());

    let mut args = CommandArgs::default();

    let command_result = match cmd_name.as_str() {
        "about" => process_command(ctx, command, args, utility::slash_about).await,
        "avatar" => process_command(ctx, command, args, osu::slash_avatar).await,
        "backgroundgame" => {
            args.bucket.replace(BucketName::BgStart);

            process_command(ctx, command, args, fun::slash_backgroundgame).await
        }
        "cache" => {
            args.only_owner = true;

            process_command(ctx, command, args, owner::slash_cache).await
        }
        "compare" => process_command(ctx, command, args, osu::slash_compare).await,
        "help" => {
            let is_authority = super::check_authority(&ctx, &command).transpose().is_none();

            help::slash_help(ctx, command, is_authority)
                .await
                .map(ProcessResult::success)
        }
        "invite" => process_command(ctx, command, args, utility::slash_invite).await,
        "link" => process_command(ctx, command, args, osu::slash_link).await,
        "matchcost" => process_command(ctx, command, args, osu::slash_matchcost).await,
        "matchlive" => {
            args.authority = true;

            process_command(ctx, command, args, osu::slash_matchlive).await
        }
        "medal" => process_command(ctx, command, args, osu::slash_medal).await,
        "minesweeper" => process_command(ctx, command, args, fun::slash_minesweeper).await,
        "mostplayed" => process_command(ctx, command, args, osu::slash_mostplayed).await,
        "ping" => process_command(ctx, command, args, utility::slash_ping).await,
        "pp" => process_command(ctx, command, args, osu::slash_pp).await,
        "profile" => process_command(ctx, command, args, osu::slash_profile).await,
        "rank" => process_command(ctx, command, args, osu::slash_rank).await,
        "ranking" => process_command(ctx, command, args, osu::slash_ranking).await,
        "ratio" => process_command(ctx, command, args, osu::slash_ratio).await,
        "recent" => process_command(ctx, command, args, osu::slash_recent).await,
        "roleassign" => {
            args.authority = true;
            args.only_guilds = true;

            process_command(ctx, command, args, utility::slash_roleassign).await
        }
        "roll" => process_command(ctx, command, args, utility::slash_roll).await,
        "search" => process_command(ctx, command, args, osu::slash_mapsearch).await,
        "snipe" => {
            args.bucket.replace(BucketName::Snipe);

            process_command(ctx, command, args, osu::slash_snipe).await
        }
        "song" => {
            args.bucket.replace(BucketName::Songs);

            process_command(ctx, command, args, songs::slash_song).await
        }
        "track" => {
            args.authority = true;
            args.only_guilds = true;

            process_command(ctx, command, args, tracking::slash_track).await
        }
        "trackstream" => {
            args.authority = true;
            args.only_guilds = true;

            process_command(ctx, command, args, twitch::slash_trackstream).await
        }
        "whatif" => process_command(ctx, command, args, osu::slash_whatif).await,
        _ => return Err(Error::UnknownSlashCommand(cmd_name)),
    };

    match command_result {
        Ok(ProcessResult::Success) => info!("Processed slash command `{}`", cmd_name),
        Ok(result) => info!("Command `/{}` was not processed: {:?}", cmd_name, result),
        Err(why) => return Err(Error::Command(Box::new(why), cmd_name)),
    }

    Ok(())
}

async fn process_command<F, R>(
    ctx: Arc<Context>,
    command: ApplicationCommand,
    args: CommandArgs,
    fun: F,
) -> BotResult<ProcessResult>
where
    F: Fn(Arc<Context>, ApplicationCommand) -> R,
    R: Future<Output = BotResult<()>>,
{
    match pre_process_command(&ctx, &command, args).await? {
        Some(result) => Ok(result),
        None => {
            // TODO: Convey to discord that the command is now being processed, maybe already earlier?

            // Call command function
            (fun)(ctx, command).await?;

            Ok(ProcessResult::Success)
        }
    }
}

#[inline(never)]
async fn pre_process_command(
    ctx: &Context,
    command: &ApplicationCommand,
    args: CommandArgs,
) -> BotResult<Option<ProcessResult>> {
    let guild_id = command.guild_id;

    // Only in guilds?
    if args.only_guilds && guild_id.is_none() {
        let content = "That command is only available in guilds";
        command.error(&ctx, content).await?;

        return Ok(Some(ProcessResult::NoDM));
    }

    let author_id = command.author().ok_or(Error::MissingSlashAuthor)?.id;

    // Only for owner?
    if args.only_owner && author_id.0 != OWNER_USER_ID {
        let content = "That command can only be used by the bot owner";
        command.error(&ctx, content).await?;

        return Ok(Some(ProcessResult::NoOwner));
    }

    let channel_id = command.channel_id;

    // Does bot have sufficient permissions to send response?
    match ctx.cache.current_user() {
        Some(bot_user) => {
            let permissions =
                ctx.cache
                    .get_channel_permissions_for(bot_user.id, channel_id, guild_id);

            if !permissions.contains(Permissions::SEND_MESSAGES) {
                debug!("No SEND_MESSAGE permission, can not respond");

                return Ok(Some(ProcessResult::NoSendPermission));
            }
        }
        None => error!("No CurrentUser in cache for permission check"),
    };

    // Ratelimited?
    {
        let guard = ctx.buckets.get(&BucketName::All).unwrap();
        let mutex = guard.value();
        let mut bucket = mutex.lock().await;
        let ratelimit = bucket.take(author_id.0);

        if ratelimit > 0 {
            debug!("Ratelimiting user {} for {} seconds", author_id, ratelimit,);

            return Ok(Some(ProcessResult::Ratelimited(BucketName::All)));
        }
    }

    if let Some(bucket) = args.bucket {
        if let Some((cooldown, bucket)) =
            super::_check_ratelimit(&ctx, author_id, guild_id, bucket).await
        {
            if !matches!(bucket, BucketName::BgHint) {
                let content = format!("Command on cooldown, try again in {} seconds", cooldown);
                command.error(&ctx, content).await?;
            }

            return Ok(Some(ProcessResult::Ratelimited(bucket)));
        }
    }

    // Only for authorities?
    if args.authority {
        match super::_check_authority(&ctx, author_id, guild_id) {
            Ok(None) => {}
            Ok(Some(content)) => {
                command.error(&ctx, content).await?;

                return Ok(Some(ProcessResult::NoAuthority));
            }
            Err(why) => {
                let content = "Error while checking authority status";
                let _ = command.error(&ctx, content).await;

                return Err(Error::Authority(Box::new(why)));
            }
        }
    }

    Ok(None)
}

fn log_slash(ctx: &Context, command: &ApplicationCommand, cmd_name: &str) {
    let username = command
        .username()
        .or_else(|| {
            command
                .member
                .as_ref()
                .and_then(|member| member.nick.as_deref())
        })
        .unwrap_or("<unknown user>");

    let mut location = String::with_capacity(31);

    match command.guild_id.and_then(|id| ctx.cache.guild(id)) {
        Some(guild) => {
            location.push_str(guild.name.as_str());
            location.push(':');

            match ctx.cache.guild_channel(command.channel_id) {
                Some(channel) => location.push_str(channel.name()),
                None => location.push_str("<uncached channel>"),
            }
        }
        None => location.push_str("Private"),
    }

    info!("[{}] {}: /{}", location, username, cmd_name);
}

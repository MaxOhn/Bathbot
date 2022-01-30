use super::ProcessResult;
use crate::{
    commands::{fun, help, osu, owner, songs, tracking, twitch, utility},
    core::buckets::BucketName,
    embeds::EmbedBuilder,
    util::{
        constants::{
            common_literals::{HELP, MAP, PROFILE},
            OWNER_USER_ID, RED,
        },
        Authored, InteractionExt,
    },
    BotResult, Context, Error,
};

use std::{future::Future, mem, sync::Arc};
use twilight_model::{
    application::{
        callback::{CallbackData, InteractionResponse},
        interaction::{ApplicationCommand, MessageComponentInteraction},
    },
    channel::message::MessageFlags,
    guild::Permissions,
};

#[derive(Copy, Clone, Default)]
struct CommandArgs {
    bools: u8,
    bucket: Option<BucketName>,
}

impl CommandArgs {
    const AUTHORITY: u8 = 1 << 0;
    const EPHEMERAL: u8 = 1 << 1;
    const ONLY_GUILDS: u8 = 1 << 2;
    const ONLY_OWNER: u8 = 1 << 3;
    // const DEFERRED_MSG: u8 = 1 << 4;

    fn authority(&self) -> bool {
        self.bools & Self::AUTHORITY > 0
    }

    fn bucket(&self) -> Option<BucketName> {
        self.bucket
    }

    fn ephemeral(&self) -> bool {
        self.bools & Self::EPHEMERAL > 0
    }

    fn only_guilds(&self) -> bool {
        self.bools & Self::ONLY_GUILDS > 0
    }

    fn only_owner(&self) -> bool {
        self.bools & Self::ONLY_OWNER > 0
    }

    fn set_authority(&mut self) {
        self.bools |= Self::AUTHORITY
    }

    fn set_ephemeral(&mut self) {
        self.bools |= Self::EPHEMERAL
    }

    fn set_only_guilds(&mut self) {
        self.bools |= Self::ONLY_GUILDS
    }

    fn set_only_owner(&mut self) {
        self.bools |= Self::ONLY_OWNER
    }
}

pub async fn handle_component(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let name = component.data.custom_id.as_str();
    log_interaction(&ctx, &*component, name);
    ctx.stats.increment_component(name);

    match name {
        "help_menu" | "help_back" => help::handle_menu_select(&ctx, *component).await,
        _ => Err(Error::UnknownMessageComponent { component }),
    }
}

pub async fn handle_autocomplete(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let name = command.data.name.as_str();
    ctx.stats.increment_autocomplete(name);

    match name {
        HELP => help::handle_autocomplete(ctx, command).await,
        _ => Err(Error::UnknownSlashAutocomplete(command.data.name)),
    }
}

pub async fn handle_command(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let name = mem::take(&mut command.data.name);
    log_interaction(&ctx, &command, &name);
    ctx.stats.increment_slash_command(&name);

    let mut args = CommandArgs::default();

    let command_result = match name.as_str() {
        "avatar" => process_command(ctx, command, args, osu::slash_avatar).await,
        "bws" => process_command(ctx, command, args, osu::slash_bws).await,
        "commands" => process_command(ctx, command, args, utility::slash_commands).await,
        "compare" => process_command(ctx, command, args, osu::slash_compare).await,
        "config" => {
            args.set_ephemeral();

            process_command(ctx, command, args, utility::slash_config).await
        }
        "fix" => process_command(ctx, command, args, osu::slash_fix).await,
        HELP => {
            // Necessary to be able to use data.create_message later on
            start_thinking_ephemeral(&ctx, &command).await?;

            help::slash_help(ctx, command)
                .await
                .map(|_| ProcessResult::Success)
        }
        "invite" => process_command(ctx, command, args, utility::slash_invite).await,
        "leaderboard" => process_command(ctx, command, args, osu::slash_leaderboard).await,
        "link" => {
            args.set_ephemeral();

            process_command(ctx, command, args, osu::slash_link).await
        }
        MAP => process_command(ctx, command, args, osu::slash_map).await,
        "mapper" => process_command(ctx, command, args, osu::slash_mapper).await,
        "matchcost" => process_command(ctx, command, args, osu::slash_matchcost).await,
        "matchlive" => {
            args.set_authority();

            process_command(ctx, command, args, osu::slash_matchlive).await
        }
        "medal" => process_command(ctx, command, args, osu::slash_medal).await,
        "minesweeper" => process_command(ctx, command, args, fun::slash_minesweeper).await,
        "mostplayed" => process_command(ctx, command, args, osu::slash_mostplayed).await,
        "nochoke" => process_command(ctx, command, args, osu::slash_nochoke).await,
        "osc" => process_command(ctx, command, args, osu::slash_osc).await,
        "osekai" => process_command(ctx, command, args, osu::slash_osekai).await,
        "osustats" => process_command(ctx, command, args, osu::slash_osustats).await,
        "owner" => {
            args.set_only_owner();
            args.set_ephemeral();

            process_command(ctx, command, args, owner::slash_owner).await
        }
        "ping" => process_command(ctx, command, args, utility::slash_ping).await,
        "pinned" => process_command(ctx, command, args, osu::slash_pinned).await,
        "pp" => process_command(ctx, command, args, osu::slash_pp).await,
        PROFILE => process_command(ctx, command, args, osu::slash_profile).await,
        "prune" => {
            args.set_authority();
            args.set_only_guilds();

            process_command(ctx, command, args, utility::slash_prune).await
        }
        "rank" => process_command(ctx, command, args, osu::slash_rank).await,
        "ranking" => process_command(ctx, command, args, osu::slash_ranking).await,
        "ratios" => process_command(ctx, command, args, osu::slash_ratio).await,
        "recent" => process_command(ctx, command, args, osu::slash_recent).await,
        "roleassign" => {
            args.set_authority();
            args.set_only_guilds();

            process_command(ctx, command, args, utility::slash_roleassign).await
        }
        "roll" => process_command(ctx, command, args, utility::slash_roll).await,
        "rb" => process_command(ctx, command, args, osu::slash_rb).await,
        "rs" => process_command(ctx, command, args, osu::slash_rs).await,
        "search" => process_command(ctx, command, args, osu::slash_mapsearch).await,
        "serverconfig" => {
            args.set_authority();
            args.set_only_guilds();

            process_command(ctx, command, args, utility::slash_serverconfig).await
        }
        "simulate" => process_command(ctx, command, args, osu::slash_simulate).await,
        "snipe" => {
            args.bucket = Some(BucketName::Snipe);

            process_command(ctx, command, args, osu::slash_snipe).await
        }
        "song" => {
            args.bucket = Some(BucketName::Songs);

            process_command(ctx, command, args, songs::slash_song).await
        }
        "top" => process_command(ctx, command, args, osu::slash_top).await,
        "topif" => process_command(ctx, command, args, osu::slash_topif).await,
        "topold" => process_command(ctx, command, args, osu::slash_topold).await,
        "track" => {
            args.set_authority();
            args.set_only_guilds();

            process_command(ctx, command, args, tracking::slash_track).await
        }
        "trackstream" => {
            args.set_authority();
            args.set_only_guilds();

            process_command(ctx, command, args, twitch::slash_trackstream).await
        }
        "whatif" => process_command(ctx, command, args, osu::slash_whatif).await,
        _ => {
            return Err(Error::UnknownSlashCommand {
                name,
                command: Box::new(command),
            });
        }
    };

    match command_result {
        Ok(ProcessResult::Success) => info!("Processed slash command `{name}`"),
        Ok(result) => info!("Command `/{name}` was not processed: {result:?}"),
        Err(why) => return Err(Error::Command(Box::new(why), name)),
    }

    Ok(())
}

async fn process_command<R>(
    ctx: Arc<Context>,
    command: ApplicationCommand,
    args: CommandArgs,
    fun: fn(Arc<Context>, ApplicationCommand) -> R,
) -> BotResult<ProcessResult>
where
    R: Future<Output = BotResult<()>>,
{
    let ephemeral = args.ephemeral();

    match pre_process_command(&ctx, &command, args).await? {
        Some(result) => Ok(result),
        None => {
            // Let discord know the command is now being processed
            if ephemeral {
                start_thinking_ephemeral(&ctx, &command).await?;
            } else {
                start_thinking(&ctx, &command).await?;
            }

            // Call command function
            (fun)(ctx, command).await?;

            Ok(ProcessResult::Success)
        }
    }
}

async fn start_thinking(ctx: &Context, command: &ApplicationCommand) -> BotResult<()> {
    let response = InteractionResponse::DeferredChannelMessageWithSource(CallbackData {
        allowed_mentions: None,
        components: None,
        content: None,
        embeds: None,
        flags: None,
        tts: None,
    });

    ctx.interaction()
        .interaction_callback(command.id, &command.token, &response)
        .exec()
        .await?;

    Ok(())
}

async fn start_thinking_ephemeral(ctx: &Context, command: &ApplicationCommand) -> BotResult<()> {
    let response = InteractionResponse::DeferredChannelMessageWithSource(CallbackData {
        allowed_mentions: None,
        components: None,
        content: None,
        embeds: None,
        flags: Some(MessageFlags::EPHEMERAL),
        tts: None,
    });

    ctx.interaction()
        .interaction_callback(command.id, &command.token, &response)
        .exec()
        .await?;

    Ok(())
}

async fn premature_error(
    ctx: &Context,
    command: &ApplicationCommand,
    content: impl Into<String>,
    ephemeral: bool,
) -> BotResult<()> {
    let embed = EmbedBuilder::new().color(RED).description(content).build();
    let flags = ephemeral.then(|| MessageFlags::EPHEMERAL);

    let response = InteractionResponse::ChannelMessageWithSource(CallbackData {
        allowed_mentions: None,
        components: None,
        content: None,
        embeds: Some(vec![embed]),
        flags,
        tts: None,
    });

    ctx.interaction()
        .interaction_callback(command.id, &command.token, &response)
        .exec()
        .await?;

    Ok(())
}

#[inline(never)]
async fn pre_process_command(
    ctx: &Context,
    command: &ApplicationCommand,
    args: CommandArgs,
) -> BotResult<Option<ProcessResult>> {
    let guild_id = command.guild_id;

    // Only in guilds?
    if args.only_guilds() && guild_id.is_none() {
        let content = "That command is only available in guilds";
        premature_error(ctx, command, content, false).await?;

        return Ok(Some(ProcessResult::NoDM));
    }

    let author_id = command.author().ok_or(Error::MissingInteractionAuthor)?.id;

    // Only for owner?
    if args.only_owner() && author_id.get() != OWNER_USER_ID {
        let content = "That command can only be used by the bot owner";
        premature_error(ctx, command, content, true).await?;

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
            premature_error(ctx, command, content, true).await?;

            return Ok(Some(ProcessResult::NoSendPermission));
        }
    }

    // Ratelimited?
    {
        let mutex = ctx.buckets.get(BucketName::All);
        let mut bucket = mutex.lock();
        let ratelimit = bucket.take(author_id.get());

        if ratelimit > 0 {
            trace!("Ratelimiting user {author_id} for {ratelimit} seconds");

            return Ok(Some(ProcessResult::Ratelimited(BucketName::All)));
        }
    }

    if let Some(bucket) = args.bucket() {
        if let Some((cooldown, bucket)) =
            super::_check_ratelimit(ctx, author_id, guild_id, bucket).await
        {
            if !matches!(bucket, BucketName::BgHint) {
                let content = format!("Command on cooldown, try again in {cooldown} seconds");
                premature_error(ctx, command, content, true).await?;
            }

            return Ok(Some(ProcessResult::Ratelimited(bucket)));
        }
    }

    // Only for authorities?
    if args.authority() {
        match super::check_authority(ctx, author_id, command.guild_id).await {
            Ok(None) => {}
            Ok(Some(content)) => {
                premature_error(ctx, command, content, true).await?;

                return Ok(Some(ProcessResult::NoAuthority));
            }
            Err(why) => {
                let content = "Error while checking authority status";
                let _ = premature_error(ctx, command, content, true).await;

                return Err(Error::Authority(Box::new(why)));
            }
        }
    }

    Ok(None)
}

fn log_interaction(ctx: &Context, interaction: &dyn InteractionExt, name: &str) {
    let username = interaction.username().unwrap_or("<unknown user>");
    let mut location = String::with_capacity(32);
    let guild = interaction.guild_id();

    match guild.and_then(|id| ctx.cache.guild(id, |g| g.name().to_owned()).ok()) {
        Some(guild_name) => {
            location.push_str(guild_name.as_str());
            location.push(':');

            let push_result = ctx
                .cache
                .channel(interaction.channel_id(), |c| location.push_str(c.name()));

            if push_result.is_err() {
                location.push_str("<uncached channel>");
            }
        }
        None => location.push_str("Private"),
    }

    info!("[{location}] {username} used `{name}` interaction");
}

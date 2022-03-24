use std::{fmt, future::Future, mem, sync::Arc};

use bitflags::bitflags;
use twilight_model::{
    application::interaction::{
        ApplicationCommand, ApplicationCommandAutocomplete, MessageComponentInteraction,
    },
    channel::message::MessageFlags,
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

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

use super::ProcessResult;

#[derive(Copy, Clone, Default)]
struct CommandArgs {
    bools: ArgBools,
    bucket: Option<BucketName>,
}

bitflags! {
    #[derive(Default)]
    pub struct ArgBools: u8 {
        const AUTHORITY    = 1 << 0;
        const EPHEMERAL    = 1 << 1;
        const ONLY_GUILDS  = 1 << 2;
        const ONLY_OWNER   = 1 << 3;
        // const DEFERRED_MSG = 1 << 4; // TODO
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
        "bg_start_include" => fun::handle_bg_start_include(&ctx, *component).await,
        "bg_start_exclude" => fun::handle_bg_start_exclude(&ctx, *component).await,
        "bg_start_effects" => fun::handle_bg_start_effects(&ctx, *component).await,
        "bg_start_button" => fun::handle_bg_start_button(ctx, *component).await,
        "bg_start_cancel" => fun::handle_bg_start_cancel(&ctx, *component).await,
        _ => Err(Error::UnknownMessageComponent { component }),
    }
}

pub async fn handle_autocomplete(
    ctx: Arc<Context>,
    command: ApplicationCommandAutocomplete,
) -> BotResult<()> {
    let name = command.data.name.as_str();
    ctx.stats.increment_autocomplete(name);

    match name {
        HELP => help::handle_autocomplete(ctx, command).await,
        "badges" => osu::handle_badge_autocomplete(ctx, command).await,
        "medal" => osu::handle_medal_autocomplete(ctx, command).await,
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
        "badges" => process_command(ctx, command, args, osu::slash_badges).await,
        // TODO: Bucket
        "bg" => process_command(ctx, command, args, fun::slash_bg).await,
        "bws" => process_command(ctx, command, args, osu::slash_bws).await,
        "commands" => process_command(ctx, command, args, utility::slash_commands).await,
        "compare" => process_command(ctx, command, args, osu::slash_compare).await,
        "config" => {
            args.bools |= ArgBools::EPHEMERAL;

            process_command(ctx, command, args, utility::slash_config).await
        }
        "countrytop" => process_command(ctx, command, args, osu::slash_countrytop).await,
        "cs" => process_command(ctx, command, args, osu::slash_cs).await,
        "fix" => process_command(ctx, command, args, osu::slash_fix).await,
        "graph" => process_command(ctx, command, args, osu::slash_graph).await,
        HELP => {
            // Necessary to be able to use data.create_message later on
            start_thinking(&ctx, &command, true).await?;

            help::slash_help(ctx, command)
                .await
                .map(|_| ProcessResult::Success)
        }
        "invite" => process_command(ctx, command, args, utility::slash_invite).await,
        "leaderboard" => process_command(ctx, command, args, osu::slash_leaderboard).await,
        "link" => {
            args.bools |= ArgBools::EPHEMERAL;

            process_command(ctx, command, args, osu::slash_link).await
        }
        MAP => process_command(ctx, command, args, osu::slash_map).await,
        "mapper" => process_command(ctx, command, args, osu::slash_mapper).await,
        "matchcompare" => {
            args.bucket = Some(BucketName::MatchCompare);

            process_command(ctx, command, args, osu::slash_matchcompare).await
        }
        "matchcost" => process_command(ctx, command, args, osu::slash_matchcost).await,
        "matchlive" => {
            args.bools |= ArgBools::AUTHORITY;

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
            args.bools |= ArgBools::ONLY_OWNER;
            args.bools |= ArgBools::EPHEMERAL;

            process_command(ctx, command, args, owner::slash_owner).await
        }
        "ping" => process_command(ctx, command, args, utility::slash_ping).await,
        "pinned" => process_command(ctx, command, args, osu::slash_pinned).await,
        "popular" => process_command(ctx, command, args, osu::slash_popular).await,
        "pp" => process_command(ctx, command, args, osu::slash_pp).await,
        PROFILE => process_command(ctx, command, args, osu::slash_profile).await,
        "prune" => {
            args.bools |= ArgBools::AUTHORITY;
            args.bools |= ArgBools::ONLY_GUILDS;

            process_command(ctx, command, args, utility::slash_prune).await
        }
        "rank" => process_command(ctx, command, args, osu::slash_rank).await,
        "ranking" => process_command(ctx, command, args, osu::slash_ranking).await,
        "ratios" => process_command(ctx, command, args, osu::slash_ratio).await,
        "recent" => process_command(ctx, command, args, osu::slash_recent).await,
        "roleassign" => {
            args.bools |= ArgBools::AUTHORITY;
            args.bools |= ArgBools::ONLY_GUILDS;

            process_command(ctx, command, args, utility::slash_roleassign).await
        }
        "roll" => process_command(ctx, command, args, utility::slash_roll).await,
        "rb" => process_command(ctx, command, args, osu::slash_rb).await,
        "rs" => process_command(ctx, command, args, osu::slash_rs).await,
        "search" => process_command(ctx, command, args, osu::slash_mapsearch).await,
        "serverconfig" => {
            args.bools |= ArgBools::AUTHORITY;
            args.bools |= ArgBools::ONLY_GUILDS;

            process_command(ctx, command, args, utility::slash_serverconfig).await
        }
        "serverleaderboard" => {
            args.bucket = Some(BucketName::Leaderboard);
            args.bools |= ArgBools::ONLY_GUILDS;

            process_command(ctx, command, args, osu::slash_serverleaderboard).await
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
            args.bools |= ArgBools::AUTHORITY;
            args.bools |= ArgBools::ONLY_GUILDS;

            process_command(ctx, command, args, tracking::slash_track).await
        }
        "trackstream" => {
            args.bools |= ArgBools::AUTHORITY;
            args.bools |= ArgBools::ONLY_GUILDS;

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
    let ephemeral = args.bools.contains(ArgBools::EPHEMERAL);

    match pre_process_command(&ctx, &command, args).await? {
        Some(result) => Ok(result),
        None => {
            // Let discord know the command is now being processed
            start_thinking(&ctx, &command, ephemeral).await?;

            // Call command function
            (fun)(ctx, command).await?;

            Ok(ProcessResult::Success)
        }
    }
}

async fn start_thinking(
    ctx: &Context,
    command: &ApplicationCommand,
    ephemeral: bool,
) -> BotResult<()> {
    let data = InteractionResponseData {
        flags: ephemeral.then(|| MessageFlags::EPHEMERAL),
        ..Default::default()
    };

    let response = InteractionResponse {
        kind: InteractionResponseType::DeferredChannelMessageWithSource,
        data: Some(data),
    };

    ctx.interaction()
        .create_response(command.id, &command.token, &response)
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

    let data = InteractionResponseData {
        embeds: Some(vec![embed]),
        flags,
        ..Default::default()
    };

    let response = InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(data),
    };

    ctx.interaction()
        .create_response(command.id, &command.token, &response)
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
    if args.bools.contains(ArgBools::ONLY_GUILDS) && guild_id.is_none() {
        let content = "That command is only available in servers";
        premature_error(ctx, command, content, false).await?;

        return Ok(Some(ProcessResult::NoDM));
    }

    let author_id = command.author().ok_or(Error::MissingInteractionAuthor)?.id;

    // Only for owner?
    if args.bools.contains(ArgBools::ONLY_OWNER) && author_id.get() != OWNER_USER_ID {
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

    if let Some(bucket) = args.bucket {
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
    if args.bools.contains(ArgBools::AUTHORITY) {
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
    let location = InteractionLocationLog { ctx, interaction };
    info!("[{location}] {username} used `{name}` interaction");
}

struct InteractionLocationLog<'l> {
    ctx: &'l Context,
    interaction: &'l dyn InteractionExt,
}

impl fmt::Display for InteractionLocationLog<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let guild = match self.interaction.guild_id() {
            Some(id) => id,
            None => return f.write_str("Private"),
        };

        match self.ctx.cache.guild(guild, |g| write!(f, "{}:", g.name())) {
            Ok(Ok(_)) => {
                let channel_result = self.ctx.cache.channel(self.interaction.channel_id(), |c| {
                    f.write_str(c.name.as_deref().unwrap_or("<uncached channel>"))
                });

                match channel_result {
                    Ok(Ok(_)) => Ok(()),
                    Ok(err) => err,
                    Err(_) => f.write_str("<uncached channel>"),
                }
            }
            Ok(err) => err,
            Err(_) => f.write_str("<uncached guild>"),
        }
    }
}

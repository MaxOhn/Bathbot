use std::{borrow::Cow, sync::Arc};

use command_macros::{command, SlashCommand};
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::{thread::AutoArchiveDuration, ChannelType},
};

use crate::{
    commands::ThreadChannel,
    core::commands::CommandOrigin,
    matchlive::MatchTrackResult,
    util::{
        builder::MessageBuilder,
        constants::{
            GENERAL_ISSUE, INVALID_ACTION_FOR_CHANNEL_TYPE, OSU_API_ISSUE, OSU_BASE,
            THREADS_UNAVAILABLE,
        },
        matcher, ApplicationCommandExt, ChannelExt,
    },
    BotResult, Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "matchlive",
    help = "Similar to what an mp link does, this command will \
    keep a channel up to date about events in a multiplayer match."
)]
#[flags(AUTHORITY)]
/// Live track a multiplayer match
pub enum Matchlive<'a> {
    #[command(name = "track")]
    Add(MatchliveAdd<'a>),
    #[command(name = "untrack")]
    Remove(MatchliveRemove<'a>),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "track")]
/// Start tracking a match
pub struct MatchliveAdd<'a> {
    /// Specify a match url or match id
    match_url: Cow<'a, str>,
    /// Choose if a new thread should be started
    thread: ThreadChannel,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "untrack")]
/// Untrack a match
pub struct MatchliveRemove<'a> {
    /// Specify a match url or match id
    match_url: Cow<'a, str>,
}

async fn slash_matchlive(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Matchlive::from_interaction(command.input_data())? {
        Matchlive::Add(args) => matchlive(ctx, command.into(), args).await,
        Matchlive::Remove(args) => matchliveremove(ctx, command.into(), Some(args)).await,
    }
}

#[command]
#[desc("Live track a multiplayer match")]
#[help(
    "Live track a multiplayer match in a channel.\n\
    Similar to what an mp link does, I will keep a channel up \
    to date about events in a match.\n\
    Use the `matchliveremove` command to stop tracking the match."
)]
#[usage("[match url / match id]")]
#[examples("58320988", "https://osu.ppy.sh/community/matches/58320988")]
#[alias("ml", "mla", "matchliveadd", "mlt", "matchlivetrack")]
#[bucket(MatchLive)]
#[flags(AUTHORITY)]
#[group(AllModes)]
async fn prefix_matchlive(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
    match args.next() {
        Some(arg) => {
            let args = MatchliveAdd {
                match_url: arg.into(),
                thread: ThreadChannel::Channel,
            };

            matchlive(ctx, msg.into(), args).await
        }
        None => {
            let content = "You must specify either a match id or a multiplayer link to a match";
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Untrack a multiplayer match")]
#[help(
    "Untrack a multiplayer match in a channel.\n\
    The match id only has to be specified in case the channel \
    currently live tracks more than one match."
)]
#[usage("[match url / match id]")]
#[examples("58320988", "https://osu.ppy.sh/community/matches/58320988")]
#[alias("mlr")]
#[flags(AUTHORITY)]
#[group(AllModes)]
async fn prefix_matchliveremove(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> BotResult<()> {
    let args = match args.next() {
        Some(arg) => match parse_match_id(arg) {
            Ok(_) => Some(MatchliveRemove {
                match_url: arg.into(),
            }),
            Err(content) => {
                msg.error(&ctx, content).await?;

                return Ok(());
            }
        },
        None => None,
    };

    matchliveremove(ctx, msg.into(), args).await
}

fn parse_match_id(match_url: &str) -> Result<u32, &'static str> {
    match matcher::get_osu_match_id(match_url) {
        Some(id) => Ok(id),
        None => {
            let content = "Failed to parse match url.\n\
                Be sure to provide either a match id or the multiplayer link to a match";

            Err(content)
        }
    }
}

async fn matchlive(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MatchliveAdd<'_>,
) -> BotResult<()> {
    let MatchliveAdd { match_url, thread } = args;

    let match_id = match parse_match_id(&match_url) {
        Ok(id) => id,
        Err(content) => return orig.error(&ctx, content).await,
    };

    let mut channel = orig.channel_id();

    if let ThreadChannel::Thread = thread {
        if orig.guild_id().is_none() {
            return orig.error(&ctx, THREADS_UNAVAILABLE).await;
        }

        let kind = ChannelType::GuildPublicThread;
        let archive_dur = AutoArchiveDuration::Day;
        let thread_name = format!("Live tracking match id {match_id}");

        let create_fut = ctx
            .http
            .create_thread(channel, &thread_name, kind)
            .unwrap()
            .auto_archive_duration(archive_dur)
            .exec();

        match create_fut.await {
            Ok(res) => channel = res.model().await?.id,
            Err(err) => {
                let content = match err.kind() {
                    ErrorType::Response {
                        error: ApiError::General(err),
                        ..
                    } => match err.code {
                        INVALID_ACTION_FOR_CHANNEL_TYPE => Some(THREADS_UNAVAILABLE),
                        _ => None,
                    },
                    _ => None,
                };

                match content {
                    Some(content) => return orig.error(&ctx, content).await,
                    None => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.into());
                    }
                }
            }
        }
    }

    let content: &str = match ctx.add_match_track(channel, match_id).await {
        MatchTrackResult::Added => match orig {
            CommandOrigin::Message { .. } => return Ok(()),
            CommandOrigin::Interaction { command } => {
                ctx.interaction()
                    .delete_response(&command.token)
                    .exec()
                    .await?;

                return Ok(());
            }
        },
        MatchTrackResult::Capped => "Channels can track at most three games at a time",
        MatchTrackResult::Duplicate => "That match is already being tracking in this channel",
        MatchTrackResult::Error => OSU_API_ISSUE,
        MatchTrackResult::NotFound => "The osu!api returned a 404 indicating an invalid match id",
        MatchTrackResult::Private => "The match can't be tracked because it is private",
    };

    orig.error(&ctx, content).await
}

async fn matchliveremove(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: Option<MatchliveRemove<'_>>,
) -> BotResult<()> {
    let channel = orig.channel_id();

    let match_id = match args.map(|args| parse_match_id(&args.match_url)) {
        Some(Ok(id)) => id,
        Some(Err(content)) => return orig.error(&ctx, content).await,
        None => match ctx.tracks_single_match(channel).await {
            Some(id) => id,
            None => {
                let content = "The channel does not track exactly one match \
                    and the match id could not be parsed from the first argument.\n\
                    Try specifying the match id as first argument.";

                return orig.error(&ctx, content).await;
            }
        },
    };

    if ctx.remove_match_track(channel, match_id).await {
        let content =
            format!("Stopped live tracking [the match]({OSU_BASE}community/matches/{match_id})",);

        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, &builder).await?;

        Ok(())
    } else {
        let content = "The match wasn't tracked in this channel";

        orig.error(&ctx, content).await
    }
}

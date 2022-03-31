use std::sync::Arc;

use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            ApplicationCommand,
        },
    },
    channel::{thread::AutoArchiveDuration, ChannelType},
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    core::MatchTrackResult,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE, OSU_BASE},
        matcher, MessageBuilder, MessageExt,
    },
    BotResult, CommandData, Context, Error,
};

#[command]
#[authority()]
#[short_desc("Live track a multiplayer match")]
#[long_desc(
    "Live track a multiplayer match in a channel.\n\
    Similar to what an mp link does, I will keep a channel up \
    to date about events in a match.\n\
    Use the `matchliveremove` command to stop tracking the match."
)]
#[usage("[match url / match id]")]
#[example("58320988", "https://osu.ppy.sh/community/matches/58320988")]
#[aliases("ml", "mla", "matchliveadd", "mlt", "matchlivetrack")]
#[bucket("match_live")]
async fn matchlive(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let match_id = match args.next().and_then(matcher::get_osu_match_id) {
                Some(arg) => arg,
                None => {
                    let content = "The first argument must be either a match \
                        id or the multiplayer link to a match";

                    return msg.error(&ctx, content).await;
                }
            };

            _matchlive(
                ctx,
                CommandData::Message { msg, args, num },
                match_id.into(),
            )
            .await
        }
        CommandData::Interaction { command } => slash_matchlive(ctx, *command).await,
    }
}

async fn _matchlive(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: MatchLiveArgs,
) -> BotResult<()> {
    let MatchLiveArgs {
        match_id,
        new_thread,
    } = args;

    let mut channel = data.channel_id();

    if new_thread {
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
            Err(err) => match handle_error(err.kind()) {
                Some(content) => return data.error(&ctx, content).await,
                None => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.into());
                }
            },
        }
    }

    let content: &str = match ctx.add_match_track(channel, match_id).await {
        MatchTrackResult::Added => match data {
            CommandData::Message { .. } => return Ok(()),
            CommandData::Interaction { command } => return command.delete_message(&ctx).await,
        },
        MatchTrackResult::Capped => "Channels can track at most three games at a time",
        MatchTrackResult::Duplicate => "That match is already being tracking in this channel",
        MatchTrackResult::Error => OSU_API_ISSUE,
        MatchTrackResult::NotFound => "The osu!api returned a 404 indicating an invalid match id",
        MatchTrackResult::Private => "The match can't be tracked because it is private",
    };

    data.error(&ctx, content).await
}

const INVALID_ACTION_FOR_CHANNEL_TYPE: u64 = 50024;

fn handle_error(kind: &ErrorType) -> Option<&'static str> {
    match kind {
        ErrorType::Response { error, .. } => match error {
            ApiError::General(err) => match err.code {
                INVALID_ACTION_FOR_CHANNEL_TYPE => Some("Cannot start new thread from here"),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

#[command]
#[authority()]
#[short_desc("Untrack a multiplayer match")]
#[long_desc(
    "Untrack a multiplayer match in a channel.\n\
    The match id only has to be specified in case the channel \
    currently live tracks more than one match."
)]
#[usage("[match url / match id]")]
#[example("58320988", "https://osu.ppy.sh/community/matches/58320988")]
#[aliases("mlr")]
async fn matchliveremove(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let match_id_opt = match args.next().and_then(matcher::get_osu_match_id) {
                Some(id) => Some(id),
                None => ctx.tracks_single_match(msg.channel_id).await,
            };

            let match_id = match match_id_opt {
                Some(match_id) => match_id,
                None => {
                    let content = "The channel does not track exactly one match \
                        and the match id could not be parsed from the first argument.\n\
                        Try specifying the match id as first argument.";

                    return msg.error(&ctx, content).await;
                }
            };

            _matchliveremove(ctx, CommandData::Message { msg, args, num }, match_id).await
        }
        CommandData::Interaction { command } => slash_matchlive(ctx, *command).await,
    }
}

async fn _matchliveremove(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    match_id: u32,
) -> BotResult<()> {
    if ctx.remove_match_track(data.channel_id(), match_id).await {
        let content =
            format!("Stopped live tracking [the match]({OSU_BASE}community/matches/{match_id})",);

        let builder = MessageBuilder::new().embed(content);
        data.create_message(&ctx, builder).await?;

        Ok(())
    } else {
        let content = "The match wasn't tracked in this channel";

        data.error(&ctx, content).await
    }
}

struct MatchLiveArgs {
    match_id: u32,
    new_thread: bool,
}

impl From<u32> for MatchLiveArgs {
    fn from(match_id: u32) -> Self {
        Self {
            match_id,
            new_thread: false,
        }
    }
}

enum MatchLiveKind {
    Add(MatchLiveArgs),
    Remove(u32),
}

fn parse_match_id(options: &[CommandDataOption]) -> BotResult<Result<u32, &'static str>> {
    let option = options.first().and_then(|option| {
        (option.name == "match_url").then(|| match &option.value {
            CommandOptionValue::String(value) => Some(value),
            _ => None,
        })
    });

    match option.flatten() {
        Some(value) => match matcher::get_osu_match_id(value.as_str()) {
            Some(id) => Ok(Ok(id)),
            None => {
                let content = "Failed to parse match url.\n\
                    Be sure it's a valid mp url or a match id";

                Ok(Err(content))
            }
        },
        None => Err(Error::InvalidCommandOptions),
    }
}

impl MatchLiveKind {
    fn slash(command: &ApplicationCommand) -> BotResult<Result<Self, &'static str>> {
        let option = command
            .data
            .options
            .first()
            .ok_or(Error::InvalidCommandOptions)?;

        match &option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "track" => {
                    let mut match_id = None;
                    let mut thread = None;

                    for option in options {
                        match &option.value {
                            CommandOptionValue::String(value) => match option.name.as_str() {
                                "match_url" => match matcher::get_osu_match_id(value.as_str()) {
                                    Some(id) => match_id = Some(id),
                                    None => {
                                        let content = "Failed to parse match url.\n\
                                            Be sure it's a valid mp url or a match id";

                                        return Ok(Err(content));
                                    }
                                },
                                "thread" => match value.as_str() {
                                    "channel" => thread = Some(false),
                                    "thread" => thread = Some(true),
                                    _ => return Err(Error::InvalidCommandOptions),
                                },
                                _ => return Err(Error::InvalidCommandOptions),
                            },
                            _ => return Err(Error::InvalidCommandOptions),
                        }
                    }

                    let args = MatchLiveArgs {
                        match_id: match_id.ok_or(Error::InvalidCommandOptions)?,
                        new_thread: thread.ok_or(Error::InvalidCommandOptions)?,
                    };

                    Ok(Ok(Self::Add(args)))
                }
                "untrack" => match parse_match_id(options)? {
                    Ok(match_id) => Ok(Ok(MatchLiveKind::Remove(match_id))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_matchlive(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    match MatchLiveKind::slash(&command)? {
        Ok(MatchLiveKind::Add(id)) => _matchlive(ctx, command.into(), id).await,
        Ok(MatchLiveKind::Remove(id)) => _matchliveremove(ctx, command.into(), id).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

fn option_match_url() -> MyCommandOption {
    MyCommandOption::builder("match_url", "Specify a match url or match id")
        .string(Vec::new(), true)
}

pub fn define_matchlive() -> MyCommand {
    let thread_choices = vec![
        CommandOptionChoice::String {
            name: "Start new thread".to_owned(),
            value: "thread".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Stay in channel".to_owned(),
            value: "channel".to_owned(),
        },
    ];

    let thread = MyCommandOption::builder("thread", "Choose if a new thread should be started")
        .string(thread_choices, true);

    let track = MyCommandOption::builder("track", "Start tracking a match")
        .subcommand(vec![option_match_url(), thread]);

    let untrack =
        MyCommandOption::builder("untrack", "Untrack a match").subcommand(vec![option_match_url()]);

    let help = "Similar to what an mp link does, this command will \
        keep a channel up to date about events in a multiplayer match.";

    MyCommand::new("matchlive", "Live track a multiplayer match")
        .help(help)
        .options(vec![track, untrack])
        .authority()
}

use std::sync::Arc;

use twilight_model::application::interaction::{
    application_command::{CommandDataOption, CommandOptionValue},
    ApplicationCommand,
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    core::MatchTrackResult,
    util::{
        constants::{OSU_API_ISSUE, OSU_BASE},
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

            _matchlive(ctx, CommandData::Message { msg, args, num }, match_id).await
        }
        CommandData::Interaction { command } => slash_matchlive(ctx, *command).await,
    }
}

async fn _matchlive(ctx: Arc<Context>, data: CommandData<'_>, match_id: u32) -> BotResult<()> {
    let content: &str = match ctx.add_match_track(data.channel_id(), match_id).await {
        MatchTrackResult::Added => match data {
            CommandData::Message { .. } => return Ok(()),
            CommandData::Interaction { command } => return command.delete_message(&ctx).await,
        },
        MatchTrackResult::Capped => "Channels can track at most three games at a time",
        MatchTrackResult::Duplicate => "That match is already being tracking in this channel",
        MatchTrackResult::Error => OSU_API_ISSUE,
        MatchTrackResult::Private => "The match can't be tracked because it is private",
    };

    data.error(&ctx, content).await
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
            let match_id_opt = args
                .next()
                .and_then(matcher::get_osu_match_id)
                .or_else(|| ctx.tracks_single_match(msg.channel_id));

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
    if ctx.remove_match_track(data.channel_id(), match_id) {
        let content = format!(
            "Stopped live tracking [the match]({OSU_BASE}community/matches/{match_id})",
        );

        let builder = MessageBuilder::new().embed(content);
        data.create_message(&ctx, builder).await?;

        Ok(())
    } else {
        let content = "The match wasn't tracked in this channel";

        data.error(&ctx, content).await
    }
}

enum MatchLiveKind {
    Add(u32),
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
                "track" => match parse_match_id(options)? {
                    Ok(match_id) => Ok(Ok(MatchLiveKind::Add(match_id))),
                    Err(content) => Ok(Err(content)),
                },
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
    let track = MyCommandOption::builder("track", "Start tracking a match")
        .subcommand(vec![option_match_url()]);

    let untrack =
        MyCommandOption::builder("untrack", "Untrack a match").subcommand(vec![option_match_url()]);

    let help = "Similar to what an mp link does, this command will \
        keep a channel up to date about events in a multiplayer match.";

    MyCommand::new("matchlive", "Live track a multiplayer match")
        .help(help)
        .options(vec![track, untrack])
        .authority()
}

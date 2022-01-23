mod common;
mod most_played;
mod profile;
mod score;

pub use common::*;
pub use most_played::*;
pub use profile::*;
pub use score::*;

use std::sync::Arc;

use rosu_v2::prelude::{GameMode, Username};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        osu::{option_discord, option_map, option_mode, option_mods, option_name},
        parse_discord, parse_mode_option, DoubleResultCow, MyCommand, MyCommandOption,
    },
    database::OsuData,
    util::{
        constants::common_literals::{MODE, PROFILE, SCORE},
        matcher, InteractionExt, MessageExt,
    },
    Args, BotResult, Context, Error,
};

use super::{prepare_score, require_link, MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32};

const AT_LEAST_ONE: &str = "You need to specify at least one osu username. \
    If you're not linked, you must specify two names.";

struct TripleArgs {
    name1: Option<Username>,
    name2: Username,
    name3: Option<Username>,
    mode: GameMode,
}

impl TripleArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
        mode: Option<GameMode>,
    ) -> DoubleResultCow<Self> {
        let name1 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(user_id) => match parse_discord(ctx, user_id).await? {
                    Ok(osu) => osu.into_username(),
                    Err(content) => return Ok(Err(content)),
                },
                None => arg.into(),
            },
            None => return Ok(Err(AT_LEAST_ONE.into())),
        };

        let mode = mode.unwrap_or(GameMode::STD);

        let name2 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(user_id) => match parse_discord(ctx, user_id).await? {
                    Ok(osu) => osu.into_username(),
                    Err(content) => return Ok(Err(content)),
                },
                None => arg.into(),
            },
            None => {
                return Ok(Ok(Self {
                    name1: ctx
                        .psql()
                        .get_user_osu(author_id)
                        .await?
                        .map(OsuData::into_username),
                    name2: name1,
                    name3: None,
                    mode,
                }))
            }
        };

        let name3 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(user_id) => match parse_discord(ctx, user_id).await? {
                    Ok(osu) => Some(osu.into_username()),
                    Err(content) => return Ok(Err(content)),
                },
                None => Some(arg.into()),
            },
            None => None,
        };

        let args = Self {
            name1: Some(name1),
            name2,
            name3,
            mode,
        };

        Ok(Ok(args))
    }

    async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut name1 = None;
        let mut name2 = None;
        let mut name3 = None;
        let mut mode = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => mode = parse_mode_option(&value),
                    "name1" => name1 = Some(value.into()),
                    "name2" => name2 = Some(value.into()),
                    "name3" => name3 = Some(value.into()),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    "discord1" => match parse_discord(ctx, value).await? {
                        Ok(osu) => name1 = Some(osu.into_username()),
                        Err(content) => return Ok(Err(content)),
                    },
                    "discord2" => match parse_discord(ctx, value).await? {
                        Ok(osu) => name2 = Some(osu.into_username()),
                        Err(content) => return Ok(Err(content)),
                    },
                    "discord3" => match parse_discord(ctx, value).await? {
                        Ok(osu) => name3 = Some(osu.into_username()),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let (name1, name2, name3) = match (name1, name2, name3) {
            (None, Some(name2), Some(name3)) => (Some(name2), name3, None),
            (name1, Some(name2), name3) => (name1, name2, name3),
            (Some(name1), None, Some(name3)) => (Some(name1), name3, None),
            (Some(name), None, None) => (None, name, None),
            (None, None, Some(name)) => (None, name, None),
            (None, None, None) => return Ok(Err(AT_LEAST_ONE.into())),
        };

        let name1 = match name1 {
            Some(name) => Some(name),
            None => ctx
                .psql()
                .get_user_osu(command.user_id()?)
                .await?
                .map(OsuData::into_username),
        };

        let args = TripleArgs {
            name1,
            name2,
            name3,
            mode: mode.unwrap_or(GameMode::STD),
        };

        Ok(Ok(args))
    }
}

enum CompareCommandKind {
    Score(ScoreArgs),
    Profile(ProfileArgs),
    Top(TripleArgs),
    Mostplayed(TripleArgs),
}

impl CompareCommandKind {
    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                SCORE => match ScoreArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Score(args))),
                    Err(content) => Ok(Err(content)),
                },
                PROFILE => match ProfileArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(CompareCommandKind::Profile(args))),
                    Err(content) => Ok(Err(content)),
                },
                "top" => match TripleArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(CompareCommandKind::Top(args))),
                    Err(content) => Ok(Err(content)),
                },
                "mostplayed" => match TripleArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(CompareCommandKind::Mostplayed(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_compare(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match CompareCommandKind::slash(&ctx, &mut command).await? {
        Ok(CompareCommandKind::Score(args)) => _compare(ctx, command.into(), args).await,
        Ok(CompareCommandKind::Profile(args)) => _profilecompare(ctx, command.into(), args).await,
        Ok(CompareCommandKind::Top(args)) => _common(ctx, command.into(), args).await,
        Ok(CompareCommandKind::Mostplayed(args)) => {
            _mostplayedcommon(ctx, command.into(), args).await
        }
        Err(msg) => command.error(&ctx, msg).await,
    }
}

fn option_name_(n: u8) -> MyCommandOption {
    let mut name = option_name();

    name.name = match n {
        1 => "name1",
        2 => "name2",
        3 => "name3",
        _ => unreachable!(),
    };

    name
}

fn option_discord_(n: u8) -> MyCommandOption {
    let mut discord = option_discord();

    discord.name = match n {
        1 => "discord1",
        2 => "discord2",
        3 => "discord3",
        _ => unreachable!(),
    };

    discord.help = if n == 1 {
        Some(
            "Instead of specifying an osu! username with the `name1` option, \
            you can use this `discord1` option to choose a discord user.\n\
            For it to work, the user must be linked to an osu! account i.e. they must have used \
            the `/link` or `/config` command to verify their account.",
        )
    } else {
        None
    };

    discord
}

pub fn define_compare() -> MyCommand {
    let name = option_name();
    let map = option_map();
    let mods = option_mods(false);
    let discord = option_discord();

    let score_help =
        "Given a user and a map, display the user's play with the most score on the map";

    let score = MyCommandOption::builder(SCORE, "Compare a score")
        .help(score_help)
        .subcommand(vec![name, map, mods, discord]);

    let mode = option_mode();
    let name1 = option_name_(1);
    let name2 = option_name_(2);
    let discord1 = option_discord_(1);
    let discord2 = option_discord_(2);

    let profile_help = "Compare profile stats between two players.\n\
        Note:\n\
        - PC peak = Monthly playcount peak\n\
        - PP spread = PP difference between the top score and the 100th score";

    let profile = MyCommandOption::builder(PROFILE, "Compare two profiles")
        .help(profile_help)
        .subcommand(vec![mode, name1, name2, discord1, discord2]);

    let mode = option_mode();
    let name1 = option_name_(1);
    let name2 = option_name_(2);
    let name3 = option_name_(3);
    let discord1 = option_discord_(1);
    let discord2 = option_discord_(2);
    let discord3 = option_discord_(3);

    let top_help = "Compare common top scores between players and see who did better on them";

    let top = MyCommandOption::builder("top", "Compare common top scores")
        .help(top_help)
        .subcommand(vec![
            mode, name1, name2, name3, discord1, discord2, discord3,
        ]);

    let mode = option_mode();
    let name1 = option_name_(1);
    let name2 = option_name_(2);
    let name3 = option_name_(3);
    let discord1 = option_discord_(1);
    let discord2 = option_discord_(2);
    let discord3 = option_discord_(3);

    let mostplayed_help = "Compare most played maps between players and see who played them more";

    let mostplayed = MyCommandOption::builder("mostplayed", "Compare most played maps")
        .help(mostplayed_help)
        .subcommand(vec![
            mode, name1, name2, name3, discord1, discord2, discord3,
        ]);

    MyCommand::new("compare", "Compare a score, top scores, or profiles")
        .options(vec![score, profile, top, mostplayed])
}

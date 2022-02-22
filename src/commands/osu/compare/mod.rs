mod common;
mod most_played;
mod profile;
mod score;

use std::{
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use eyre::Report;
use hashbrown::HashMap;
use rosu::prelude::{GameMode as GameModeV1, Score};
use rosu_v2::prelude::{GameMode, Username};
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            ApplicationCommand,
        },
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        osu::{option_discord, option_map, option_mode, option_name},
        parse_discord, parse_mode_option, DoubleResultCow, MyCommand, MyCommandOption,
    },
    database::OsuData,
    pp::PpCalculator,
    util::{
        constants::common_literals::{ACC, ACCURACY, COMBO, MODE, PROFILE, SCORE, SORT},
        matcher, InteractionExt, MessageExt,
    },
    Args, BotResult, Context, Error,
};

pub use self::{common::*, most_played::*, profile::*, score::*};

use super::require_link;

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

#[derive(Copy, Clone)]
enum ScoreOrder {
    Acc,
    Combo,
    Date,
    Misses,
    Pp,
    Score,
    Stars,
}

impl Default for ScoreOrder {
    fn default() -> Self {
        Self::Score
    }
}

impl ScoreOrder {
    pub async fn apply(self, ctx: &Context, scores: &mut [Score], map_id: u32, mode: GameModeV1) {
        if scores.len() <= 1 {
            return;
        }

        match self {
            Self::Acc => {
                scores.sort_unstable_by(|a, b| {
                    b.accuracy(mode)
                        .partial_cmp(&a.accuracy(mode))
                        .unwrap_or(Ordering::Equal)
                });
            }
            Self::Combo => scores.sort_unstable_by_key(|s| Reverse(s.max_combo)),
            Self::Date => scores.sort_unstable_by_key(|s| Reverse(s.date)),
            Self::Misses => scores.sort_unstable_by(|a, b| {
                b.count_miss.cmp(&a.count_miss).then_with(|| {
                    let hits_a = a.total_hits(mode);
                    let hits_b = b.total_hits(mode);

                    let ratio_a = a.count_miss as f32 / hits_a as f32;
                    let ratio_b = b.count_miss as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
            }),
            Self::Pp => {
                let mut calc = match PpCalculator::new(ctx, map_id).await {
                    Ok(calc) => calc,
                    Err(err) => {
                        warn!("{:?}", Report::new(err));

                        return;
                    }
                };

                let pp = scores
                    .iter()
                    .map(|score| {
                        let id = score.date.timestamp();

                        (id, calc.score(score).pp() as f32)
                    })
                    .collect::<HashMap<_, _>>();

                scores.sort_unstable_by(|a, b| {
                    let id_a = a.date.timestamp();

                    let pp_a = match pp.get(&id_a) {
                        Some(pp) => pp,
                        None => return Ordering::Greater,
                    };

                    let id_b = b.date.timestamp();

                    let pp_b = match pp.get(&id_b) {
                        Some(pp) => pp,
                        None => return Ordering::Less,
                    };

                    pp_b.partial_cmp(pp_a).unwrap_or(Ordering::Equal)
                })
            }
            Self::Score => scores.sort_unstable_by_key(|s| Reverse(s.score)),
            Self::Stars => {
                let mut calc = match PpCalculator::new(ctx, map_id).await {
                    Ok(calc) => calc,
                    Err(err) => {
                        warn!("{:?}", Report::new(err));

                        return;
                    }
                };

                let stars = scores
                    .iter()
                    .map(|score| {
                        let id = score.date.timestamp();

                        (id, calc.score(score).stars() as f32)
                    })
                    .collect::<HashMap<_, _>>();

                scores.sort_unstable_by(|a, b| {
                    let id_a = a.date.timestamp();

                    let stars_a = match stars.get(&id_a) {
                        Some(stars) => stars,
                        None => return Ordering::Greater,
                    };

                    let id_b = b.date.timestamp();

                    let stars_b = match stars.get(&id_b) {
                        Some(stars) => stars,
                        None => return Ordering::Less,
                    };

                    stars_b.partial_cmp(stars_a).unwrap_or(Ordering::Equal)
                })
            }
        }
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

fn score_options() -> Vec<MyCommandOption> {
    let name = option_name();
    let map = option_map();
    let discord = option_discord();

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "date".to_owned(),
            value: "date".to_owned(),
        },
        CommandOptionChoice::String {
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: COMBO.to_owned(),
            value: COMBO.to_owned(),
        },
        CommandOptionChoice::String {
            name: "stars".to_owned(),
            value: "stars".to_owned(),
        },
        CommandOptionChoice::String {
            name: "misses".to_owned(),
            value: "miss".to_owned(),
        },
        CommandOptionChoice::String {
            name: "score".to_owned(),
            value: "score".to_owned(),
        },
    ];

    let sort = MyCommandOption::builder(SORT, "Choose how the scores should be ordered")
        .help("Choose how the scores should be ordered, defaults to `score`.")
        .string(sort_choices, false);

    vec![name, map, sort, discord]
}

pub fn define_compare() -> MyCommand {
    let score_help = "Given a user and a map, display the user's scores on the map.\n\
        Its shorter alias is the `/cs` command.";

    let score = MyCommandOption::builder(SCORE, "Compare a score (same as `/cs`)")
        .help(score_help)
        .subcommand(score_options());

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

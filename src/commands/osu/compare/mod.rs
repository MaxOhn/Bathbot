use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use command_macros::{HasName, SlashCommand};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, Score, Username};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{pp::PpCalculator, util::matcher, BotResult, Context};

pub use self::{common::*, most_played::*, profile::*, score::*};

mod common;
mod most_played;
mod profile;
mod score;

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

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "compare")]
/// Compare scores or profiles
pub enum Compare<'a> {
    Score(CompareScore<'a>),
    Profile(CompareProfile<'a>),
    Top(CompareTop<'a>),
    MostPlayed(CompareMostPlayed<'a>),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "score",
    help = "Given a user and a map, display the user's scores on the map.\n\
        Its shorter alias is the `/cs` command."
)]
/// Compare a score (same as `/cs`)
pub struct CompareScore<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(help = "Specify a map either by map url or map id.\n\
    If none is specified, it will search in the recent channel history \
    and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<Cow<'a, str>>,
    /// Choose how the scores should be ordered
    sort: Option<CompareScoreOrder>,
    #[command(help = "Filter out scores based on mods.\n\
        Mods must be given as `+mods` to require these mods to be included, \
        `+mods!` to require exactly these mods, \
        or `-mods!` to ignore scores containing any of these mods.\n\
        Examples:\n\
        - `+hd`: Remove scores that don't include `HD`\n\
        - `+hdhr!`: Only keep the `HDHR` score\n\
        - `+nm!`: Only keep the nomod score\n\
        - `-ezhd!`: Remove all scores that have either `EZ` or `HD`")]
    /// Filter out scores based on mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)
    mods: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

// TODO: Use util::osu::ScoreOrder instead?
#[derive(CommandOption, CreateOption)]
pub enum CompareScoreOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

impl Default for CompareScoreOrder {
    fn default() -> Self {
        Self::Score
    }
}

impl CompareScoreOrder {
    pub async fn apply(self, ctx: &Context, scores: &mut [Score], map_id: u32) {
        if scores.len() <= 1 {
            return;
        }

        match self {
            Self::Acc => {
                scores.sort_unstable_by(|a, b| {
                    b.accuracy
                        .partial_cmp(&a.accuracy)
                        .unwrap_or(Ordering::Equal)
                });
            }
            Self::Combo => scores.sort_unstable_by_key(|s| Reverse(s.max_combo)),
            Self::Date => scores.sort_unstable_by_key(|s| Reverse(s.created_at)),
            Self::Misses => scores.sort_unstable_by(|a, b| {
                b.statistics
                    .count_miss
                    .cmp(&a.statistics.count_miss)
                    .then_with(|| {
                        let hits_a = a.total_hits();
                        let hits_b = b.total_hits();

                        let ratio_a = a.statistics.count_miss as f32 / hits_a as f32;
                        let ratio_b = b.statistics.count_miss as f32 / hits_b as f32;

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
                        let id = score.created_at.timestamp();

                        (id, calc.score(score).pp() as f32)
                    })
                    .collect::<HashMap<_, _>>();

                scores.sort_unstable_by(|a, b| {
                    let id_a = a.created_at.timestamp();

                    let pp_a = match pp.get(&id_a) {
                        Some(pp) => pp,
                        None => return Ordering::Greater,
                    };

                    let id_b = b.created_at.timestamp();

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
                        let id = score.created_at.timestamp();

                        (id, calc.score(score).stars() as f32)
                    })
                    .collect::<HashMap<_, _>>();

                scores.sort_unstable_by(|a, b| {
                    let id_a = a.created_at.timestamp();

                    let stars_a = match stars.get(&id_a) {
                        Some(stars) => stars,
                        None => return Ordering::Greater,
                    };

                    let id_b = b.created_at.timestamp();

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

#[derive(CommandMode, CreateCommand)]
#[command(
    name = "profile",
    help = "Compare profile stats between two players.\n\
        Note:\n\
        - PC peak = Monthly playcount peak\n\
        - PP spread = PP difference between the top score and the 100th score"
)]
/// Compare two profiles
pub struct CompareProfile<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name1: Option<Cow<'a, str>>,
    /// Specify a username
    name2: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord1: Option<Id<UserMarker>>,
    /// Specify a linked discord user
    discord2: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "top",
    help = "Compare common top scores between players and see who did better on them"
)]
/// Compare common top scores
pub struct CompareTop<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name1: Option<Cow<'a, str>>,
    /// Specify a username
    name2: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord1: Option<Id<UserMarker>>,
    /// Specify a linked discord user
    discord2: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "mostplayed",
    help = "Compare most played maps between players and see who played them more"
)]
/// Compare most played maps
pub struct CompareMostPlayed<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name1: Option<Cow<'a, str>>,
    /// Specify a username
    name2: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord1: Option<Id<UserMarker>>,
    /// Specify a linked discord user
    discord2: Option<Id<UserMarker>>,
}

async fn slash_compare(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Compare::from_interaction(command.input_data())? {
        Compare::Score(args) => score(ctx, command.into(), args).await,
        Compare::Profile(args) => profile(ctx, command.into(), args).await,
        Compare::Top(args) => top(ctx, command.into(), args).await,
        Compare::MostPlayed(args) => mostplayed(ctx, command.into(), args).await,
    }
}

use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use command_macros::SlashCommand;
use eyre::Result;
use rosu_v2::prelude::Score;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::GameModeOption,
    pp::PpCalculator,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub use self::{common::*, most_played::*, profile::*, score::*};

mod common;
mod most_played;
mod profile;
mod score;

const AT_LEAST_ONE: &str = "You need to specify at least one osu username. \
    If you're not linked, you must specify two names.";

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "compare")]
/// Compare scores or profiles
pub enum Compare<'a> {
    #[command(name = "score")]
    Score(CompareScore<'a>),
    #[command(name = "profile")]
    Profile(CompareProfile<'a>),
    #[command(name = "top")]
    Top(CompareTop<'a>),
    #[command(name = "mostplayed")]
    MostPlayed(CompareMostPlayed<'a>),
}

#[derive(CommandModel, CreateCommand)]
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
        Self::Pp
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
            Self::Date => scores.sort_unstable_by_key(|s| Reverse(s.ended_at)),
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
                let calc = match PpCalculator::new(ctx, map_id).await {
                    Ok(calc) => calc,
                    Err(err) => {
                        warn!("{:?}", err.wrap_err("Failed to get pp calculator"));

                        return;
                    }
                };

                let f = |score: &mut Score| {
                    let pp = calc.score(score).pp() as f32;
                    let _ = score.pp.get_or_insert(pp);

                    pp
                };

                sort_by_cached_key(scores, f);
            }
            Self::Score => scores.sort_unstable_by_key(|s| Reverse(s.score)),
            Self::Stars => {
                let calc = match PpCalculator::new(ctx, map_id).await {
                    Ok(calc) => calc,
                    Err(err) => {
                        warn!("{:?}", err.wrap_err("Failed to get pp calculator"));

                        return;
                    }
                };

                let f = |score: &mut Score| calc.score(&*score).stars() as f32;
                sort_by_cached_key(scores, f);
            }
        }
    }
}

/// Modified `slice::sort_by_cached_key`
fn sort_by_cached_key<F>(slice: &mut [Score], f: F)
where
    F: FnMut(&mut Score) -> f32,
{
    let mut indices: Vec<_> = slice
        .iter_mut()
        .map(f)
        .enumerate()
        .map(|(i, v)| (v, i as u32))
        .collect();

    indices.sort_unstable_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap_or(Ordering::Equal));

    for i in 0..slice.len() {
        let mut idx = indices[i].1;

        while (idx as usize) < i {
            idx = indices[idx as usize].1;
        }

        indices[i].1 = idx;
        slice.swap(i, idx as usize);
    }
}

#[derive(CommandModel, CreateCommand, Default)]
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

#[derive(CommandModel, CreateCommand, Default)]
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

#[derive(CommandModel, CreateCommand, Default)]
#[command(
    name = "mostplayed",
    help = "Compare most played maps between players and see who played them more"
)]
/// Compare most played maps
pub struct CompareMostPlayed<'a> {
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

async fn slash_compare(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Compare::from_interaction(command.input_data())? {
        Compare::Score(args) => match CompareScoreArgs::try_from(args) {
            Ok(args) => score(ctx, (&mut command).into(), args).await,
            Err(content) => {
                command.error(&ctx, content).await?;

                Ok(())
            }
        },
        Compare::Profile(args) => profile(ctx, (&mut command).into(), args).await,
        Compare::Top(args) => top(ctx, (&mut command).into(), args).await,
        Compare::MostPlayed(args) => mostplayed(ctx, (&mut command).into(), args).await,
    }
}

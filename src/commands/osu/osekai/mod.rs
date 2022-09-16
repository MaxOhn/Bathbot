use std::sync::Arc;

use command_macros::SlashCommand;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    custom_client::{
        Badges, LovedMapsets, RankedMapsets, Replays, StandardDeviation, Subscribers, TotalPp,
    },
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

use self::{
    medal_count::medal_count,
    rarity::rarity,
    user_value::{count, pp},
};

use super::UserValue;

mod medal_count;
mod rarity;
mod user_value;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "osekai",
    help = "Various leaderboard stats. \
    All data is provided by [osekai](https://osekai.net/)."
)]
/// Various leaderboards provided by osekai
pub enum Osekai {
    #[command(name = "badges")]
    Badges(OsekaiBadges),
    #[command(name = "loved_mapsets")]
    LovedMapsets(OsekaiLovedMapsets),
    #[command(name = "medal_count")]
    MedalCount(OsekaiMedalCount),
    #[command(name = "ranked_mapsets")]
    RankedMapsets(OsekaiRankedMapsets),
    #[command(name = "rarity")]
    Rarity(OsekaiRarity),
    #[command(name = "replays")]
    Replays(OsekaiReplays),
    #[command(name = "standard_deviation")]
    StandardDeviation(OsekaiStandardDeviation),
    #[command(name = "subscribers")]
    Subscribers(OsekaiSubscribers),
    #[command(name = "total_pp")]
    TotalPp(OsekaiTotalPp),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "badges")]
/// Who has the most profile badges?
pub struct OsekaiBadges {
    /// If specified, only show users of this country
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "loved_mapsets")]
/// Who created the most loved mapsets?
pub struct OsekaiLovedMapsets {
    /// If specified, only show users of this country
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "medal_count")]
/// Who has the most medals?
pub struct OsekaiMedalCount {
    /// If specified, only show users of this country
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "ranked_mapsets")]
/// Who created the most ranked mapsets?
pub struct OsekaiRankedMapsets {
    /// If specified, only show users of this country
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "rarity")]
/// What are the rarest medals?
pub struct OsekaiRarity;

#[derive(CommandModel, CreateCommand)]
#[command(name = "replays")]
/// Who has the most replays watched?
pub struct OsekaiReplays {
    /// If specified, only show users of this country
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "standard_deviation",
    help = "Who has the highest pp \
    [standard deviation](https://en.wikipedia.org/wiki/Standard_deviation) across all modes?"
)]
/// Who has the highest pp standard deviation across all modes?
pub struct OsekaiStandardDeviation {
    /// If specified, only show users of this country
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "subscribers")]
/// Which mapper has the most subscribers?
pub struct OsekaiSubscribers {
    /// If specified, only show users of this country
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "total_pp")]
/// Who has the highest total pp in all modes combined?
pub struct OsekaiTotalPp {
    /// If specified, only show users of this country
    country: Option<String>,
}

async fn slash_osekai(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Osekai::from_interaction(command.input_data())? {
        Osekai::Badges(args) => count::<Badges>(ctx, command, args.country).await,
        Osekai::LovedMapsets(args) => count::<LovedMapsets>(ctx, command, args.country).await,
        Osekai::MedalCount(args) => medal_count(ctx, command, args).await,
        Osekai::RankedMapsets(args) => count::<RankedMapsets>(ctx, command, args.country).await,
        Osekai::Rarity(_) => rarity(ctx, command).await,
        Osekai::Replays(args) => count::<Replays>(ctx, command, args.country).await,
        Osekai::StandardDeviation(args) => {
            pp::<StandardDeviation>(ctx, command, args.country).await
        }
        Osekai::Subscribers(args) => count::<Subscribers>(ctx, command, args.country).await,
        Osekai::TotalPp(args) => pp::<TotalPp>(ctx, command, args.country).await,
    }
}

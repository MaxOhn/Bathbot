use std::sync::Arc;

use command_macros::SlashCommand;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    custom_client::{
        Badges, LovedMapsets, RankedMapsets, Replays, StandardDeviation, Subscribers, TotalPp,
    },
    util::ApplicationCommandExt,
    BotResult, Context,
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
pub struct OsekaiBadges;

#[derive(CommandModel, CreateCommand)]
#[command(name = "loved_mapsets")]
/// Who created the most loved mapsets?
pub struct OsekaiLovedMapsets;

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
pub struct OsekaiRankedMapsets;

#[derive(CommandModel, CreateCommand)]
#[command(name = "rarity")]
/// What are the rarest medals?
pub struct OsekaiRarity;

#[derive(CommandModel, CreateCommand)]
#[command(name = "replays")]
/// Who has the most replays watched?
pub struct OsekaiReplays;

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "standard_deviation",
    help = "Who has the highest pp \
    [standard deviation](https://en.wikipedia.org/wiki/Standard_deviation) across all modes?"
)]
/// Who has the highest pp standard deviation across all modes?
pub struct OsekaiStandardDeviation;

#[derive(CommandModel, CreateCommand)]
#[command(name = "subscribers")]
/// Which mapper has the most subscribers?
pub struct OsekaiSubscribers;

#[derive(CommandModel, CreateCommand)]
#[command(name = "total_pp")]
/// Who has the highest total pp in all modes combined?
pub struct OsekaiTotalPp;

async fn slash_osekai(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Osekai::from_interaction(command.input_data())? {
        Osekai::Badges(_) => count::<Badges>(ctx, command).await,
        Osekai::LovedMapsets(_) => count::<LovedMapsets>(ctx, command).await,
        Osekai::MedalCount(args) => medal_count(ctx, command, args).await,
        Osekai::RankedMapsets(_) => count::<RankedMapsets>(ctx, command).await,
        Osekai::Rarity(_) => rarity(ctx, command).await,
        Osekai::Replays(_) => count::<Replays>(ctx, command).await,
        Osekai::StandardDeviation(_) => pp::<StandardDeviation>(ctx, command).await,
        Osekai::Subscribers(_) => count::<Subscribers>(ctx, command).await,
        Osekai::TotalPp(_) => pp::<TotalPp>(ctx, command).await,
    }
}

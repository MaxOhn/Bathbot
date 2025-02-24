use bathbot_macros::SlashCommand;
use bathbot_model::{
    Badges, LovedMapsets, RankedMapsets, Replays, StandardDeviation, Subscribers, TotalPp,
};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};

use self::{
    medal_count::medal_count,
    rarity::rarity,
    user_value::{count, pp},
};
use crate::util::{InteractionCommandExt, interaction::InteractionCommand};

mod medal_count;
mod rarity;
mod user_value;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "osekai",
    desc = "Various leaderboards provided by osekai",
    help = "Various leaderboard stats. \
    All data is provided by [osekai](https://osekai.net/)."
)]
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
#[command(name = "badges", desc = "Who has the most profile badges?")]
pub struct OsekaiBadges {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "loved_mapsets", desc = "Who created the most loved mapsets?")]
pub struct OsekaiLovedMapsets {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "medal_count", desc = "Who has the most medals?")]
pub struct OsekaiMedalCount {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "ranked_mapsets", desc = "Who created the most ranked mapsets?")]
pub struct OsekaiRankedMapsets {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "rarity", desc = "What are the rarest medals?")]
pub struct OsekaiRarity;

#[derive(CommandModel, CreateCommand)]
#[command(name = "replays", desc = "Who has the most replays watched?")]
pub struct OsekaiReplays {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "standard_deviation",
    desc = "Who has the highest pp standard deviation across all modes?",
    help = "Who has the highest pp \
    [standard deviation](https://en.wikipedia.org/wiki/Standard_deviation) across all modes?"
)]
pub struct OsekaiStandardDeviation {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "subscribers", desc = "Which mapper has the most subscribers?")]
pub struct OsekaiSubscribers {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "total_pp",
    desc = "Who has the highest total pp in all modes combined?"
)]
pub struct OsekaiTotalPp {
    #[command(desc = "If specified, only show users of this country")]
    country: Option<String>,
}

async fn slash_osekai(mut command: InteractionCommand) -> Result<()> {
    match Osekai::from_interaction(command.input_data())? {
        Osekai::Badges(args) => count::<Badges>(command, args.country).await,
        Osekai::LovedMapsets(args) => count::<LovedMapsets>(command, args.country).await,
        Osekai::MedalCount(args) => medal_count(command, args).await,
        Osekai::RankedMapsets(args) => count::<RankedMapsets>(command, args.country).await,
        Osekai::Rarity(_) => rarity(command).await,
        Osekai::Replays(args) => count::<Replays>(command, args.country).await,
        Osekai::StandardDeviation(args) => pp::<StandardDeviation>(command, args.country).await,
        Osekai::Subscribers(args) => count::<Subscribers>(command, args.country).await,
        Osekai::TotalPp(args) => pp::<TotalPp>(command, args.country).await,
    }
}

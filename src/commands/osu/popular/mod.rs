use std::sync::Arc;

use command_macros::SlashCommand;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{core::Context, BotResult, util::ApplicationCommandExt};

pub use self::mapsets::MapsetEntry;

use self::{mappers::*, maps::*, mapsets::*, mods::*};

mod mappers;
mod maps;
mod mapsets;
mod mods;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "popular",
    help = "Check out the most popular map(set)s, mods, or mappers.\n\
        All data is provided by [nzbasic](https://osu.ppy.sh/users/9008211)'s \
        website [osutracker](https://osutracker.com/)."
)]
/// Check out the most popular map(set)s, mods, or mappers
pub enum Popular {
    #[command(name = "maps")]
    Maps(PopularMaps),
    #[command(name = "mapsets")]
    Mapsets(PopularMapsets),
    #[command(name = "mods")]
    Mods(PopularMods),
    #[command(name = "mappers")]
    Mappers(PopularMappers),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "maps")]
/// What are the most common maps per pp range?
pub struct PopularMaps {
    /// Specify a pp range
    pp: PopularMapsPp,
}

#[derive(CommandOption, CreateOption)]
pub enum PopularMapsPp {
    #[option(name = "100-200pp", value = "100_200")]
    Pp100,
    #[option(name = "200-300pp", value = "200_300")]
    Pp200,
    #[option(name = "300-400pp", value = "300_400")]
    Pp300,
    #[option(name = "400-500pp", value = "400_500")]
    Pp400,
    #[option(name = "500-600pp", value = "500_600")]
    Pp500,
    #[option(name = "600-700pp", value = "600_700")]
    Pp600,
    #[option(name = "700-800pp", value = "700_800")]
    Pp700,
    #[option(name = "800-900pp", value = "800_900")]
    Pp800,
    #[option(name = "900-1000pp", value = "900_1000")]
    Pp900,
    #[option(name = "1000-1100pp", value = "1000_1100")]
    Pp1000,
    #[option(name = "1100-1200pp", value = "1100_1200")]
    Pp1100,
    #[option(name = "1200-1300pp", value = "1200_1300")]
    Pp1200,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "mapsets")]
/// What mapsets appear the most in people's top100?
pub struct PopularMapsets;

#[derive(CommandModel, CreateCommand)]
#[command(name = "mods")]
/// What mods appear the most in people's top100?
pub struct PopularMods;

#[derive(CommandModel, CreateCommand)]
#[command(name = "mappers")]
/// What mappers' mapsets appear the most in people's top100?
pub struct PopularMappers;

async fn slash_popular(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Popular::from_interaction(command.input_data())? {
        Popular::Maps(args) => maps(ctx, command, args.pp).await,
        Popular::Mapsets(_) => mapsets(ctx, command).await,
        Popular::Mods(_) => mods(ctx, command).await,
        Popular::Mappers(_) => mappers(ctx, command).await,
    }
}

use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand};
use bathbot_model::RelaxPlayersDataResponse;
use bathbot_util::{
    AuthorBuilder, constants::RELAX as RELAX_URL, numbers::WithComma, osu::flag_url,
};
use eyre::Result;
use profile::relax_profile;
use top::relax_top;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{Id, marker::UserMarker};

use crate::{
    active::impls::relax::top::RelaxTopOrder,
    commands::{DISCORD_OPTION_DESC, DISCORD_OPTION_HELP},
    manager::redis::osu::CachedUser,
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

pub mod profile;
pub mod top;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "relax",
    desc = "Relax leaderboards related data",
    help = "Relax leaderboards data, provided by [Relaxation Vault](https://rx.stanr.info/)"
)]
pub enum Relax<'a> {
    #[command(name = "profile")]
    Profile(RelaxProfile<'a>),
    #[command(name = "top")]
    Top(RelaxTop<'a>),
}

const RX_PROFILE_DESC: &str = "Show user's relax profile";
const RX_PROFILE_HELP: &str =
    "Show user's relax profile, as provided by [Relaxation Vault](https://rx.stanr.info/)";

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "profile", desc = RX_PROFILE_DESC, help = RX_PROFILE_HELP)]
pub struct RelaxProfile<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

const RX_TOP_DESC: &str = "Show user's relax top plays";
const RX_TOP_HELP: &str =
    "Show user's relax top plays, as provided by [Relaxation Vault](https://rx.stanr.info/)";

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "top", desc = RX_TOP_DESC, help = RX_TOP_HELP)]
pub struct RelaxTop<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
    #[command(desc = "Choose by which order the scores should be sorted")]
    sort: Option<RelaxTopOrder>,
}

pub async fn slash_relax(mut command: InteractionCommand) -> Result<()> {
    match Relax::from_interaction(command.input_data())? {
        Relax::Profile(args) => relax_profile((&mut command).into(), args).await,
        Relax::Top(args) => relax_top((&mut command).into(), args).await,
    }
}

pub fn relax_author_builder(
    cached_user: &CachedUser,
    relax_user: &RelaxPlayersDataResponse,
) -> AuthorBuilder {
    let text = format!(
        "{username}: {pp}pp (#{global_rank} {country_code}{country_rank})",
        username = cached_user.username,
        pp = WithComma::new(relax_user.total_pp.unwrap_or_default()),
        global_rank = relax_user.rank.unwrap_or_default(),
        country_code = cached_user.country_code.as_str(),
        country_rank = relax_user.country_rank.unwrap_or_default()
    );

    let url = format!("{RELAX_URL}/users/{}", cached_user.user_id);
    let icon = flag_url(&cached_user.country_code);

    AuthorBuilder::new(text).url(url).icon_url(icon)
}

use std::borrow::Cow;

use bathbot_macros::command;
use bathbot_model::{Countries, SnipeCountryListOrder};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    CowUtils,
};
use eyre::{Report, Result};
use rosu_v2::{
    model::GameMode,
    prelude::{CountryCode, OsuError},
    request::UserId,
};

use super::{SnipeCountryList, SnipeGameMode};
use crate::{
    active::{impls::SnipeCountryListPagination, ActiveMessages},
    commands::osu::user_not_found,
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{osu::UserArgs, RedisData},
    util::ChannelExt,
    Context,
};

#[command]
#[desc("Sort the country's #1 leaderboard")]
#[help(
    "Sort the country's #1 leaderboard.\n\
    To specify a country, you must provide its acronym e.g. `be`.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
     - `count` to sort by #1 count\n\
     - `pp` to sort by average pp of #1 scores\n\
     - `stars` to sort by average star rating of #1 scores\n\
     - `weighted` to sort by pp gained only from #1 scores\n\
    If no ordering is specified, it defaults to `count`.\n\
    If no country is specified either, I will take the country of the linked user.\n\
    Data for osu!standard originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[country acronym] [sort=count/pp/stars/weighted]")]
#[example("sort=stars", "fr sort=weighted", "sort=pp")]
#[aliases("csl", "countrysnipeleaderboard", "cslb")]
#[group(Osu)]
async fn prefix_countrysnipelist(msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipeCountryList::args(args, GameMode::Osu) {
        Ok(args) => country_list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Sort the country's ctb #1 leaderboard")]
#[help(
    "Sort the country's ctb #1 leaderboard.\n\
    To specify a country, you must provide its acronym e.g. `be`.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
     - `count` to sort by #1 count\n\
     - `pp` to sort by average pp of #1 scores\n\
     - `stars` to sort by average star rating of #1 scores\n\
     - `weighted` to sort by pp gained only from #1 scores\n\
    If no ordering is specified, it defaults to `count`.\n\
    If no country is specified either, I will take the country of the linked user.\n\
    Data for osu!catch originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[country acronym] [sort=count/pp/stars/weighted]")]
#[example("sort=stars", "fr sort=weighted", "sort=pp")]
#[aliases(
    "cslc",
    "countrysnipelistcatch",
    "countrysnipeleaderboardctb",
    "countrysnipeleaderboardcatch",
    "cslbc"
)]
#[group(Catch)]
async fn prefix_countrysnipelistctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipeCountryList::args(args, GameMode::Catch) {
        Ok(args) => country_list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Sort the country's mania #1 leaderboard")]
#[help(
    "Sort the country's mania #1 leaderboard.\n\
    To specify a country, you must provide its acronym e.g. `be`.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
     - `count` to sort by #1 count\n\
     - `pp` to sort by average pp of #1 scores\n\
     - `stars` to sort by average star rating of #1 scores\n\
     - `weighted` to sort by pp gained only from #1 scores\n\
    If no ordering is specified, it defaults to `count`.\n\
    If no country is specified either, I will take the country of the linked user.\n\
    Data for osu!mania originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[country acronym] [sort=count/pp/stars/weighted]")]
#[example("sort=stars", "fr sort=weighted", "sort=pp")]
#[aliases("cslm", "countrysnipeleaderboardmania", "cslbm")]
#[group(Mania)]
async fn prefix_countrysnipelistmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipeCountryList::args(args, GameMode::Mania) {
        Ok(args) => country_list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

pub(super) async fn country_list(
    orig: CommandOrigin<'_>,
    args: SnipeCountryList<'_>,
) -> Result<()> {
    let author_id = orig.user_id()?;

    let SnipeCountryList {
        mode,
        country,
        sort,
    } = args;

    let (osu_user, mode) = match Context::user_config().with_osu_id(author_id).await {
        Ok(config) => {
            let mode = match mode {
                Some(mode) => mode.into(),
                None => config.mode.unwrap_or(GameMode::Osu),
            };

            match config.osu {
                Some(user_id) => {
                    let user_args = UserArgs::user_id(user_id).mode(mode);

                    match Context::redis().osu_user(user_args).await {
                        Ok(user) => (Some(user), mode),
                        Err(OsuError::NotFound) => {
                            let content = user_not_found(UserId::Id(user_id)).await;

                            return orig.error(content).await;
                        }
                        Err(err) => {
                            let _ = orig.error(OSU_API_ISSUE).await;
                            let err = Report::new(err).wrap_err("failed to get user");

                            return Err(err);
                        }
                    }
                }
                None => (None, mode),
            }
        }
        Err(err) => {
            warn!("{err:?}");

            (None, GameMode::Osu)
        }
    };

    let country_code = match country {
        Some(ref country) => match Countries::name(country).to_code() {
            Some(code) => CountryCode::from(code),
            None if country.len() == 2 => CountryCode::from(country.as_ref()),
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                return orig.error(content).await;
            }
        },
        None => match &osu_user {
            Some(RedisData::Original(user)) => user.country_code.as_str().into(),
            Some(RedisData::Archive(user)) => user.country_code.as_str().into(),
            None => {
                let content = "Since you're not linked, you must specify a country (code)";

                return orig.error(content).await;
            }
        },
    };

    // Check if huisemetbenen supports the country
    if !Context::huismetbenen()
        .is_supported(country_code.as_str(), mode)
        .await
    {
        let content = format!("The country code `{country_code}` is not supported :(",);

        return orig.error(content).await;
    }

    let sort = sort.unwrap_or_default();

    // Request players
    let players = match Context::client()
        .get_snipe_country(&country_code, sort, mode)
        .await
    {
        Ok(players) => players,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get snipe country"));
        }
    };

    // Try to find author in list
    let author_idx = osu_user.as_ref().and_then(|user| {
        let author_name = user.username();

        players
            .iter()
            .position(|player| player.username == author_name)
    });

    // Enumerate players
    let players: Vec<_> = players
        .into_iter()
        .enumerate()
        .map(|(idx, player)| (idx + 1, player))
        .collect();

    let country = Countries::code(&country_code)
        .to_name()
        .map(|name| (name, country_code));

    let pagination = SnipeCountryListPagination::builder()
        .players(players.into_boxed_slice())
        .country(country)
        .order(sort)
        .author_idx(author_idx)
        .msg_owner(author_id)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

impl<'m> SnipeCountryList<'m> {
    fn args(args: Args<'m>, mode: GameMode) -> Result<Self, Cow<'static, str>> {
        let mut country = None;
        let mut sort = None;

        for arg in args.take(2).map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "sort" => {
                        sort = match value {
                            "count" => Some(SnipeCountryListOrder::Count),
                            "pp" => Some(SnipeCountryListOrder::AvgPp),
                            "stars" => Some(SnipeCountryListOrder::AvgStars),
                            "weighted" | "weightedpp" => Some(SnipeCountryListOrder::WeightedPp),
                            _ => {
                                let content = "Failed to parse `sort`. \
                                    Must be either `count`, `pp`, `stars`, or `weighted`.";

                                return Err(content.into());
                            }
                        };
                    }
                    _ => {
                        let content =
                            format!("Unrecognized option `{key}`.\nAvailable options are: `sort`.");

                        return Err(content.into());
                    }
                }
            } else {
                country = Some(arg);
            }
        }

        Ok(Self {
            mode: SnipeGameMode::try_from_mode(mode),
            country,
            sort,
        })
    }
}

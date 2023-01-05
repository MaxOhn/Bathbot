use std::{borrow::Cow, cmp::Ordering::Equal, sync::Arc};

use bathbot_macros::command;
use bathbot_model::{CountryCode, SnipeCountryPlayer as SCP};
use bathbot_util::{
    constants::{HUISMETBENEN_ISSUE, OSU_API_ISSUE},
    CowUtils,
};
use eyre::{Report, Result};
use rosu_v2::{prelude::OsuError, request::UserId};

use crate::{
    commands::osu::user_not_found,
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{osu::UserArgs, RedisData},
    pagination::CountrySnipeListPagination,
    util::ChannelExt,
    Context,
};

use super::{SnipeCountryList, SnipeCountryListOrder};

#[command]
#[desc("Sort the country's #1 leaderboard")]
#[help(
    "Sort the country's #1 leaderboard.\n\
    To specify a country, you must provide its acronym e.g. `be` \
    or alternatively you can provide `global`.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
     - `count` to sort by #1 count\n \
     - `pp` to sort by average pp of #1 scores\n \
     - `stars` to sort by average star rating of #1 scores\n \
     - `weighted` to sort by pp gained only from #1 scores\n\
    If no ordering is specified, it defaults to `count`.\n\
    If no country is specified either, I will take the country of the linked user.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[country acronym] [sort=count/pp/stars/weighted]")]
#[example("global sort=stars", "fr sort=weighted", "sort=pp")]
#[aliases("csl", "countrysnipeleaderboard", "cslb")]
#[group(Osu)]
async fn prefix_countrysnipelist(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipeCountryList::args(args) {
        Ok(args) => country_list(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

pub(super) async fn country_list(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: SnipeCountryList<'_>,
) -> Result<()> {
    let author_id = orig.user_id()?;

    // Retrieve author's osu user to check if they're in the list
    let osu_user = match ctx.user_config().osu_id(author_id).await {
        Ok(Some(user_id)) => {
            let user_args = UserArgs::user_id(user_id);

            match ctx.redis().osu_user(user_args).await {
                Ok(user) => Some(user),
                Err(OsuError::NotFound) => {
                    let content = user_not_found(&ctx, UserId::Id(user_id)).await;

                    return orig.error(&ctx, content).await;
                }
                Err(err) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                    let err = Report::new(err).wrap_err("failed to get user");

                    return Err(err);
                }
            }
        }
        Ok(None) => None,
        Err(err) => {
            warn!("{err:?}");

            None
        }
    };

    let SnipeCountryList { country, sort } = args;

    let country_code = match country {
        Some(country) => match CountryCode::from_name(&country) {
            Some(code) => code,
            None => {
                if country.len() == 2 {
                    CountryCode::from(country)
                } else {
                    let content = format!(
                        "Looks like `{country}` is neither a country name nor a country code"
                    );

                    return orig.error(&ctx, content).await;
                }
            }
        },
        None => match &osu_user {
            Some(RedisData::Original(user)) => user.country_code.as_str().into(),
            Some(RedisData::Archived(user)) => user.country_code.as_str().into(),
            None => {
                let content = "Since you're not linked, you must specify a country (code)";

                return orig.error(&ctx, content).await;
            }
        },
    };

    // Check if huisemetbenen supports the country
    if !ctx.huismetbenen().is_supported(country_code.as_str()).await {
        let content = format!("The country code `{country_code}` is not supported :(",);

        return orig.error(&ctx, content).await;
    }

    // Request players
    let mut players = match ctx.client().get_snipe_country(&country_code).await {
        Ok(players) => players,
        Err(err) => {
            let _ = orig.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(err.wrap_err("failed to get snipe country"));
        }
    };

    // Sort players
    let sort = sort.unwrap_or_default();

    let sorter = match sort {
        SnipeCountryListOrder::Count => |p1: &SCP, p2: &SCP| p2.count_first.cmp(&p1.count_first),
        SnipeCountryListOrder::Pp => {
            |p1: &SCP, p2: &SCP| p2.avg_pp.partial_cmp(&p1.avg_pp).unwrap_or(Equal)
        }
        SnipeCountryListOrder::Stars => {
            |p1: &SCP, p2: &SCP| p2.avg_sr.partial_cmp(&p1.avg_sr).unwrap_or(Equal)
        }
        SnipeCountryListOrder::WeightedPp => {
            |p1: &SCP, p2: &SCP| p2.pp.partial_cmp(&p1.pp).unwrap_or(Equal)
        }
    };

    players.sort_unstable_by(sorter);

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

    let country = ctx
        .huismetbenen()
        .get_country(country_code.as_ref())
        .await
        .map(|name| (name, country_code.as_ref().into()));

    CountrySnipeListPagination::builder(players, country, sort, author_idx)
        .start_by_update()
        .start(ctx, orig)
        .await
}

impl<'m> SnipeCountryList<'m> {
    fn args(args: Args<'m>) -> Result<Self, Cow<'static, str>> {
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
                            "pp" => Some(SnipeCountryListOrder::Pp),
                            "stars" => Some(SnipeCountryListOrder::Stars),
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

        Ok(Self { country, sort })
    }
}

use std::{borrow::Cow, cmp::Ordering::Equal, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError};

use crate::{
    commands::osu::{get_user_cached, UserArgs},
    custom_client::SnipeCountryPlayer as SCP,
    database::OsuData,
    embeds::{CountrySnipeListEmbed, EmbedData},
    pagination::{CountrySnipeListPagination, Pagination},
    util::{
        constants::{common_literals::SORT, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        numbers, CountryCode, CowUtils, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

#[command]
#[short_desc("Sort the country's #1 leaderboard")]
#[long_desc(
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
#[bucket("snipe")]
async fn countrysnipelist(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match CountryListArgs::args(&ctx, &mut args)
        {
            Ok(list_args) => {
                _countrysnipelist(ctx, CommandData::Message { msg, args, num }, list_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => super::slash_snipe(ctx, *command).await,
    }
}

pub(super) async fn _countrysnipelist(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: CountryListArgs,
) -> BotResult<()> {
    let author_id = data.author()?.id;

    // Retrieve author's osu user to check if they're in the list
    let osu_user = match ctx
        .psql()
        .get_user_osu(author_id)
        .await
        .map(|osu| osu.map(OsuData::into_username))
    {
        Ok(Some(name)) => {
            let user_args = UserArgs::new(name.as_str(), GameMode::STD);

            match get_user_cached(&ctx, &user_args).await {
                Ok(user) => Some(user),
                Err(OsuError::NotFound) => {
                    let content = format!("User `{name}` was not found");

                    return data.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            }
        }
        Ok(None) => None,
        Err(why) => {
            let wrap = format!("failed to get UserConfig for user author_id");
            warn!("{:?}", Report::new(why).wrap_err(wrap));

            None
        }
    };

    let CountryListArgs { country, sort } = args;

    let country_code = match country {
        Some(country) => country,
        None => match osu_user {
            Some(ref user) => {
                if ctx.contains_country(user.country_code.as_str()) {
                    user.country_code.as_str().into()
                } else {
                    let content = format!(
                        "`{}`'s country {} is not supported :(",
                        user.username, user.country
                    );

                    return data.error(&ctx, content).await;
                }
            }
            None => {
                let content =
                    "Since you're not linked, you must specify a country acronym, e.g. `fr`";

                return data.error(&ctx, content).await;
            }
        },
    };

    // Request players
    let mut players = match ctx.clients.custom.get_snipe_country(&country_code).await {
        Ok(players) => players,
        Err(why) => {
            let _ = data.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(why.into());
        }
    };

    // Sort players
    let sorter = match sort {
        SnipeOrder::Count => |p1: &SCP, p2: &SCP| p2.count_first.cmp(&p1.count_first),
        SnipeOrder::Pp => |p1: &SCP, p2: &SCP| p2.avg_pp.partial_cmp(&p1.avg_pp).unwrap_or(Equal),
        SnipeOrder::Stars => {
            |p1: &SCP, p2: &SCP| p2.avg_sr.partial_cmp(&p1.avg_sr).unwrap_or(Equal)
        }
        SnipeOrder::WeightedPp => |p1: &SCP, p2: &SCP| p2.pp.partial_cmp(&p1.pp).unwrap_or(Equal),
    };

    players.sort_unstable_by(sorter);

    // Try to find author in list
    let author_idx = osu_user.and_then(|user| {
        players
            .iter()
            .position(|player| player.username == user.username)
    });

    // Enumerate players
    let players: Vec<_> = players
        .into_iter()
        .enumerate()
        .map(|(idx, player)| (idx + 1, player))
        .collect();

    // Prepare embed
    let pages = numbers::div_euclid(10, players.len());
    let init_players = players.iter().take(10);

    let country = ctx
        .get_country(country_code.as_str())
        .map(|name| (name, country_code));

    let embed_data =
        CountrySnipeListEmbed::new(country.as_ref(), sort, init_players, author_idx, (1, pages));

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response = data.create_message(&ctx, builder).await?.model().await?;

    // Pagination
    let pagination = CountrySnipeListPagination::new(response, players, country, sort, author_idx);
    let owner = author_id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SnipeOrder {
    Count,
    Pp,
    Stars,
    WeightedPp,
}

impl Default for SnipeOrder {
    fn default() -> Self {
        Self::Count
    }
}

pub(super) struct CountryListArgs {
    pub country: Option<CountryCode>,
    pub sort: SnipeOrder,
}

impl CountryListArgs {
    fn args(ctx: &Context, args: &mut Args<'_>) -> Result<Self, Cow<'static, str>> {
        let mut country = None;
        let mut sort = None;

        for arg in args.take(2).map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    SORT => {
                        sort = match value {
                            "count" => Some(SnipeOrder::Count),
                            "pp" => Some(SnipeOrder::Pp),
                            "stars" => Some(SnipeOrder::Stars),
                            "weighted" | "weightedpp" => Some(SnipeOrder::WeightedPp),
                            _ => {
                                let content = "Failed to parse `sort`. \
                                    Must be either `count`, `pp`, `stars`, or `weighted`.";

                                return Err(content.into());
                            }
                        };
                    }
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\nAvailable options are: `sort`."
                        );

                        return Err(content.into());
                    }
                }
            } else if matches!(arg.as_ref(), "global" | "world") {
                country = Some("global".into());
            } else if arg.len() == 2 && arg.is_ascii() {
                let code = arg.to_uppercase();

                if !ctx.contains_country(&code) {
                    let content = format!("The country acronym `{code}` is not supported :(");

                    return Err(content.into());
                }

                country = Some(code.into())
            } else if let Some(code) = CountryCode::from_name(arg.as_ref()) {
                if !code.snipe_supported(ctx) {
                    let content = format!("The country `{code}` is not supported :(");

                    return Err(content.into());
                }

                country = Some(code);
            } else {
                let content = format!(
                    "Failed to parse `{arg}`.\n\
                    It must be either a valid country, a two ASCII character country code or \
                    `sort=count/pp/stars/weighted`"
                );

                return Err(content.into());
            }
        }

        let sort = sort.unwrap_or_default();

        Ok(Self { country, sort })
    }
}

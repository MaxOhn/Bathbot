use crate::{
    arguments::Args,
    custom_client::SnipeCountryPlayer as SCP,
    embeds::{CountrySnipeListEmbed, EmbedData},
    pagination::{CountrySnipeListPagination, Pagination},
    unwind_error,
    util::{
        constants::{HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt, SNIPE_COUNTRIES,
    },
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{cmp::Ordering::Equal, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("Sort the country's #1 leaderboard")]
#[long_desc(
    "Sort the country's #1 leaderboard.\n\
    As first argument, provide either `global`, or a country acronym, e.g. `be`.\n\
    As second argument, provide either\n\
     - `count` to sort by #1 count\n \
     - `pp` to sort by average pp of #1 scores\n \
     - `stars` to sort by average star rating of #1 scores\n \
     - `weighted pp` to sort by pp gained only from #1 scores\n\
    If no ordering is specified, it defaults to `count`.\n\
    If no country is specified either, I will take the country of the linked user.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[country acronym] [count / pp / stars / weighted pp]")]
#[example("global stars", "fr weighted pp", "be")]
#[aliases("csl", "countrysnipeleaderboard", "cslb")]
#[bucket("snipe")]
async fn countrysnipelist(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    // Retrieve author's osu user to check if they're in the list
    let osu_user = match ctx.get_link(msg.author.id.0) {
        Some(name) => match ctx.osu().user(name.as_str()).mode(GameMode::STD).await {
            Ok(Some(user)) => Some(user),
            Ok(None) => {
                let content = format!("Could not find user `{}`", name);
                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                return Err(why.into());
            }
        },
        None => None,
    };

    // Parse country acronym
    let country = match args.next() {
        Some(arg) => match arg {
            "global" | "world" => String::from("global"),
            _ => {
                if arg.len() != 2 || arg.chars().count() != 2 {
                    let content =
                        "The first argument must be a country acronym of length two, e.g. `fr`";
                    return msg.error(&ctx, content).await;
                }
                match SNIPE_COUNTRIES.get(&arg.to_uppercase()) {
                    Some(country) => country.snipe.clone(),
                    None => {
                        let content = format!("The country acronym `{}` is not supported :(", arg);
                        return msg.error(&ctx, content).await;
                    }
                }
            }
        },
        None => match osu_user {
            Some(ref user) => match SNIPE_COUNTRIES.get(&user.country) {
                Some(country) => country.snipe.to_owned(),
                None => {
                    let content = format!(
                        "`{}`'s country {} is not supported :(",
                        user.username, user.country
                    );
                    return msg.error(&ctx, content).await;
                }
            },
            None => {
                let content =
                    "Since you're not linked, you must specify a country acronym, e.g. `fr`";
                return msg.error(&ctx, content).await;
            }
        },
    };

    // Parse ordering
    let ordering = match args.next() {
        None | Some("count") => SnipeOrder::Count,
        Some("pp") => SnipeOrder::PP,
        Some("stars") | Some("sr") => SnipeOrder::Stars,
        Some("wpp") | Some("weighted pp") => SnipeOrder::WeightedPP,
        Some("weighted") => match args.next() {
            Some("pp") => SnipeOrder::WeightedPP,
            _ => {
                let content = "Following the country acronym, the next argument \
                must be either `count`, `pp`, `stars`, or `weighted pp`";
                return msg.error(&ctx, content).await;
            }
        },
        _ => {
            let content = "Following the country acronym, the next argument \
            must be either `count`, `pp`, `stars`, or `weighted pp`";
            return msg.error(&ctx, content).await;
        }
    };

    // Request players
    let mut players = match ctx.clients.custom.get_snipe_country(&country).await {
        Ok(players) => players,
        Err(why) => {
            let _ = msg.error(&ctx, HUISMETBENEN_ISSUE).await;
            return Err(why.into());
        }
    };

    // Sort players
    let sorter = match ordering {
        SnipeOrder::Count => |p1: &SCP, p2: &SCP| p2.count_first.cmp(&p1.count_first),
        SnipeOrder::PP => |p1: &SCP, p2: &SCP| p2.avg_pp.partial_cmp(&p1.avg_pp).unwrap_or(Equal),
        SnipeOrder::Stars => {
            |p1: &SCP, p2: &SCP| p2.avg_sr.partial_cmp(&p1.avg_sr).unwrap_or(Equal)
        }
        SnipeOrder::WeightedPP => |p1: &SCP, p2: &SCP| p2.pp.partial_cmp(&p1.pp).unwrap_or(Equal),
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
    let country = SNIPE_COUNTRIES
        .iter()
        .find(|(_, c)| c.snipe == country)
        .map(|(_, country)| country);
    let data = CountrySnipeListEmbed::new(country, ordering, init_players, author_idx, (1, pages));

    // Creating the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Pagination
    let pagination =
        CountrySnipeListPagination::new(response, players, country, ordering, author_idx);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (countrysnipelist): {}")
        }
    });
    Ok(())
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SnipeOrder {
    Count,
    PP,
    Stars,
    WeightedPP,
}

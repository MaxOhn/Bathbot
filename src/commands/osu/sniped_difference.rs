use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, SnipedDiffEmbed},
    pagination::{Pagination, SnipedDiffPagination},
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt, SNIPE_COUNTRIES,
    },
    BotResult, Context,
};

use chrono::{Duration, Utc};
use rosu::model::GameMode;
use std::collections::HashMap;
use std::sync::Arc;
use twilight_model::channel::Message;

async fn sniped_diff_main(
    diff: Difference,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Request the user
    let user = match ctx.osu().user(name.as_str()).mode(GameMode::STD).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("Could not find user `{}`", name);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    if SNIPE_COUNTRIES.contains_key(user.country.as_str()) {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country
        );

        return msg.error(&ctx, content).await;
    }

    let client = &ctx.clients.custom;
    let now = Utc::now();
    let week_ago = now - Duration::weeks(1);

    // Request the scores
    let scores_fut = match diff {
        Difference::Gain => client.get_national_snipes(&user, true, week_ago, now),
        Difference::Loss => client.get_national_snipes(&user, false, week_ago, now),
    };

    let scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(why.into());
        }
    };

    if scores.is_empty() {
        let content = format!(
            "`{name}` didn't {diff} national #1s in the last week.",
            name = user.username,
            diff = match diff {
                Difference::Gain => "gain any new",
                Difference::Loss => "lose any",
            }
        );

        return msg.respond(&ctx, content).await;
    }

    // Get the first five maps from the database
    let map_ids: Vec<_> = scores
        .iter()
        .take(5)
        .map(|score| score.beatmap_id)
        .collect();

    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            unwind_error!(warn, why, "Error while getting maps from DB: {}");
            HashMap::default()
        }
    };

    // Retrieving all missing beatmaps
    for map_id in map_ids {
        if !maps.contains_key(&map_id) {
            match ctx.osu().beatmap().map_id(map_id).await {
                Ok(Some(map)) => {
                    maps.insert(map_id, map);
                }
                Ok(None) => {
                    let content = format!("The API returned no beatmap for map id {}", map_id);

                    return msg.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            }
        }
    }

    let total = scores.len();
    let pages = numbers::div_euclid(5, total);

    let content = format!(
        "{name}{plural} national #1 {diff} from last week",
        name = user.username,
        plural = if user.username.ends_with('s') {
            ""
        } else {
            "s"
        },
        diff = match diff {
            Difference::Gain => "gains",
            Difference::Loss => "losses",
        }
    );

    let data_fut = SnipedDiffEmbed::new(&user, diff, scores.iter().take(5), total, (1, pages));

    let data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination =
        SnipedDiffPagination::new(Arc::clone(&ctx), response, user, diff, scores, maps);

    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (sniped_difference): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's recently acquired national #1 scores")]
#[long_desc(
    "Display a user's national #1 scores that they acquired within the last week.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("sg", "snipegain", "snipesgain")]
#[bucket("snipe")]
async fn snipedgain(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    sniped_diff_main(Difference::Gain, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's recently lost national #1 scores")]
#[long_desc(
    "Display a user's national #1 scores that they lost within the last week.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "sl",
    "snipeloss",
    "snipesloss",
    "snipedlost",
    "snipelost",
    "snipeslost"
)]
#[bucket("snipe")]
async fn snipedloss(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    sniped_diff_main(Difference::Loss, ctx, msg, args).await
}

#[derive(Copy, Clone)]
pub enum Difference {
    Gain,
    Loss,
}

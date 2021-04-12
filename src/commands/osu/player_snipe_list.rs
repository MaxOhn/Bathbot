use super::request_user;
use crate::{
    arguments::{Args, SnipeScoreArgs},
    custom_client::SnipeScoreParams,
    embeds::{EmbedData, PlayerSnipeListEmbed},
    pagination::{Pagination, PlayerSnipeListPagination},
    util::{
        constants::{HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        numbers,
        osu::ModSelection,
        MessageExt,
    },
    BotResult, Context,
};

use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, OsuError};
use std::{collections::BTreeMap, fmt::Write, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[bucket("snipe")]
#[short_desc("List all national #1 scores of a player")]
#[long_desc(
    "List all national #1 scores of a player.\n\
    There are several available orderings:\n \
    - `--acc` (`--a`): Sort by accuracy\n \
    - `--stars` (`--s`): Sort by the map's stars\n \
    - `--misses` (`--m`): Sort by amount of misses\n \
    - `--length` (`--l`): Sort by the map's length\n \
    - `--scoredate` (`--sd`): Sort by the date when the score was set\n \
    - `--mapdate` (`--md`): Sort by the map's ranked/loved date\n \
    - **None**: Sort by the score's pp.\n\
    By default the scores are sorted in descending order. To reverse, specify `--asc`.\n\
    Mods can also be specified.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username] [+mods] [--a/--md/--m/--sd/--s/--l] [--asc]")]
#[example("badewanne3 +dt --a --asc", "+hdhr --sd")]
#[aliases("psl")]
async fn playersnipelist(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = SnipeScoreArgs::new(args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let user = match request_user(&ctx, &name, Some(GameMode::STD)).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let country = if ctx.contains_country(user.country_code.as_str()) {
        user.country_code.to_owned()
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return msg.error(&ctx, content).await;
    };

    let params = SnipeScoreParams::new(user.user_id, country)
        .order(args.order)
        .descending(args.descending)
        .mods(args.mods);

    let scores_fut = ctx.clients.custom.get_national_firsts(&params);
    let count_fut = ctx.clients.custom.get_national_firsts_count(&params);

    let (scores, count) = match tokio::try_join!(scores_fut, count_fut) {
        Ok((scores, count)) => {
            let scores = scores.into_iter().enumerate().collect::<BTreeMap<_, _>>();

            (scores, count)
        }
        Err(why) => {
            let _ = msg.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(why.into());
        }
    };

    // Get the first five maps from the database
    let map_ids: Vec<_> = scores
        .values()
        .take(5)
        .map(|score| score.beatmap_id as i32)
        .collect();

    let mut maps = match ctx.psql().get_beatmaps(&map_ids, true).await {
        Ok(maps) => maps,
        Err(why) => {
            unwind_error!(warn, why, "Error while getting maps from DB: {}");

            HashMap::default()
        }
    };

    // Retrieving all missing beatmaps
    for map_id in map_ids {
        let map_id = map_id as u32;

        if !maps.contains_key(&map_id) {
            match ctx.osu().beatmap().map_id(map_id).await {
                Ok(map) => {
                    maps.insert(map_id, map);
                }
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            }
        }
    }

    let pages = numbers::div_euclid(5, count);
    let data = PlayerSnipeListEmbed::new(&user, &scores, &maps, count, (1, pages)).await;

    let mut content = format!(
        "`Order: {order:?} {descending}`",
        order = params.order,
        descending = if params.descending { "Desc" } else { "Asc" },
    );

    if let Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) = params.mods {
        let _ = write!(content, " ~ `Mods: {}`", mods,);
    }

    // Creating the embed
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(data.into_builder().build())?
        .await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = PlayerSnipeListPagination::new(
        Arc::clone(&ctx),
        response,
        user,
        scores,
        maps,
        count,
        params,
    );

    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (playersnipelist): {}")
        }
    });

    Ok(())
}

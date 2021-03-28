use super::request_user;
use crate::{
    arguments::{Args, OsuStatsArgs},
    custom_client::OsuStatsScore,
    embeds::{EmbedData, OsuStatsGlobalsEmbed},
    pagination::{OsuStatsGlobalsPagination, Pagination},
    util::{constants::OSU_API_ISSUE, numbers, osu::ModSelection, MessageExt},
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{collections::BTreeMap, fmt::Write, sync::Arc};
use twilight_model::channel::Message;

async fn osustats_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let name = ctx.get_link(msg.author.id.0);

    // Parse arguments
    let mut params = match OsuStatsArgs::new(&ctx, args, name, mode) {
        Ok(args) => args.params,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    // Retrieve user
    let user = match request_user(&ctx, params.username.as_str(), Some(mode)).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", params.username);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Retrieve their top global scores
    params.username = user.username.as_str().into();
    let (scores, amount) = match ctx.clients.custom.get_global_scores(&params).await {
        Ok((scores, amount)) => (
            scores
                .into_iter()
                .enumerate()
                .collect::<BTreeMap<usize, OsuStatsScore>>(),
            amount,
        ),
        Err(why) => {
            let content = "Some issue with the osustats website, blame bade";
            let _ = msg.error(&ctx, content).await;

            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, amount);
    let data = OsuStatsGlobalsEmbed::new(&user, &scores, amount, (1, pages)).await;

    let mut content = format!(
        "`Rank: {rank_min} - {rank_max}` ~ \
        `Acc: {acc_min}% - {acc_max}%` ~ \
        `Order: {order} {descending}`",
        acc_min = params.acc_min,
        acc_max = params.acc_max,
        rank_min = params.rank_min,
        rank_max = params.rank_max,
        order = params.order,
        descending = if params.descending { "Desc" } else { "Asc" },
    );

    if let Some(selection) = params.mods {
        let _ = write!(
            content,
            " ~ `Mods: {}`",
            match selection {
                ModSelection::Exact(mods) => mods.to_string(),
                ModSelection::Exclude(mods) => format!("Exclude {}", mods),
                ModSelection::Include(mods) => format!("Include {}", mods),
            },
        );
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
    let pagination =
        OsuStatsGlobalsPagination::new(Arc::clone(&ctx), response, user, scores, amount, params);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (osustatsglobals): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("All scores of a player that are on a map's global leaderboard")]
#[long_desc(
    "Show all scores of a player that are on a map's global leaderboard.\n\
    Rank and accuracy range can be specified with `-r` and `-a`. \
    After this keyword, you must specify either a number for max rank/acc, \
    or two numbers of the form `a..b` for min and max rank/acc.\n\
    There are several available orderings: Accuracy with `--a`, combo with `--c`, \
    pp with `--p`, rank with `--r`, score with `--s`, misses with `--m`, \
    and the default: date.\n\
    By default the scores are sorted in descending order. To reverse, specify `--asc`.\n\
    Mods can also be specified.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username] [mods] [-a [num..]num] [-r [num..]num] [--a/--c/--p/--r/--s/--m] [--asc]")]
#[example(
    "badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc",
    "vaxei +hdhr -r 1..5 --r"
)]
#[aliases("osg", "osustatsglobal")]
pub async fn osustatsglobals(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("All scores of a player that are on a map's global leaderboard")]
#[long_desc(
    "Show all scores of a player that are on a mania map's global leaderboard.\n\
    Rank and accuracy range can be specified with `-r` and `-a`. \
    After this keyword, you must specify either a number for max rank/acc, \
    or two numbers of the form `a..b` for min and max rank/acc.\n\
    There are several available orderings: Accuracy with `--a`, combo with `--c`, \
    pp with `--p`, rank with `--r`, score with `--s`, misses with `--m`, \
    and the default: date.\n\
    By default the scores are sorted in descending order. To reverse, specify `--asc`.\n\
    Mods can also be specified.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username] [mods] [-a [num..]num] [-r [num..]num] [--a/--c/--p/--r/--s/--m] [--asc]")]
#[example(
    "badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc",
    "vaxei +hdhr -r 1..5 --r"
)]
#[aliases("osgm", "osustatsglobalmania")]
pub async fn osustatsglobalsmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("All scores of a player that are on a map's global leaderboard")]
#[long_desc(
    "Show all scores of a player that are on a taiko map's global leaderboard.\n\
    Rank and accuracy range can be specified with `-r` and `-a`. \
    After this keyword, you must specify either a number for max rank/acc, \
    or two numbers of the form `a..b` for min and max rank/acc.\n\
    There are several available orderings: Accuracy with `--a`, combo with `--c`, \
    pp with `--p`, rank with `--r`, score with `--s`, misses with `--m`, \
    and the default: date.\n\
    By default the scores are sorted in descending order. To reverse, specify `--asc`.\n\
    Mods can also be specified.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username] [mods] [-a [num..]num] [-r [num..]num] [--a/--c/--p/--r/--s/--m] [--asc]")]
#[example(
    "badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc",
    "vaxei +hdhr -r 1..5 --r"
)]
#[aliases("osgt", "osustatsglobaltaiko")]
pub async fn osustatsglobalstaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("All scores of a player that are on a map's global leaderboard")]
#[long_desc(
    "Show all scores of a player that are on a ctb map's global leaderboard.\n\
    Rank and accuracy range can be specified with `-r` and `-a`. \
    After this keyword, you must specify either a number for max rank/acc, \
    or two numbers of the form `a..b` for min and max rank/acc.\n\
    There are several available orderings: Accuracy with `--a`, combo with `--c`, \
    pp with `--p`, rank with `--r`, score with `--s`, misses with `--m`, \
    and the default: date.\n\
    By default the scores are sorted in descending order. To reverse, specify `--asc`.\n\
    Mods can also be specified.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username] [mods] [-a [num..]num] [-r [num..]num] [--a/--c/--p/--r/--s/--m] [--asc]")]
#[example(
    "badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc",
    "vaxei +hdhr -r 1..5 --r"
)]
#[aliases("osgc", "osustatsglobalctb")]
pub async fn osustatsglobalsctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::CTB, ctx, msg, args).await
}

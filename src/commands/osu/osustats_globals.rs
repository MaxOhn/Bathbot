use crate::{
    arguments::{Args, OsuStatsArgs},
    bail,
    custom_client::OsuStatsScore,
    embeds::{EmbedData, OsuStatsGlobalsEmbed},
    pagination::{OsuStatsGlobalsPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers,
        osu::ModSelection,
        MessageExt,
    },
    BotResult, Context,
};

use rosu::{ models::GameMode};
use std::{collections::BTreeMap, fmt::Write, sync::Arc};
use twilight::model::channel::Message;

async fn osustats_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let name = ctx.get_link(msg.author.id.0);
    let params = match OsuStatsArgs::new(args, name, mode) {
        Ok(args) => args.params,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    // Retrieve user and their top global scores
    let (user_result, scores_result) = tokio::join!(
        ctx.osu_user(&params.username, mode),
        ctx.clients.custom.get_global_scores(&params)
    );
    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", params.username);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            msg.error(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };
    let (scores, amount) = match scores_result {
        Ok((scores, amount)) => (
            scores
                .into_iter()
                .enumerate()
                .collect::<BTreeMap<usize, OsuStatsScore>>(),
            amount,
        ),
        Err(why) => {
            msg.error(&ctx, OSU_API_ISSUE).await?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, amount);
    let data = match OsuStatsGlobalsEmbed::new(&ctx, &user, &scores, amount, (1, pages)).await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("Error while creating embed: {}", why);
        }
    };
    let mut content = format!(
        "`Acc: {acc_min}% - {acc_max}%` ~ \
        `Rank: {rank_min} - {rank_max}` ~ \
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
    let embed = data.build().build();
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
        OsuStatsGlobalsPagination::new(ctx.clone(), response, user, scores, amount, params);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

#[command]
#[short_desc("All scores of a player that are on map's global leaderboard")]
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
#[example("badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc")]
#[example("vaxei +hdhr -r 1..5 --r")]
#[aliases("osg")]
pub async fn osustatsglobals(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("All scores of a player that are on map's global leaderboard")]
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
#[example("badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc")]
#[example("vaxei +hdhr -r 1..5 --r")]
#[aliases("osgm")]
pub async fn osustatsglobalsmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("All scores of a player that are on map's global leaderboard")]
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
#[example("badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc")]
#[example("vaxei +dtmr -r 1..5 --r")]
#[aliases("osgt")]
pub async fn osustatsglobalstaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("All scores of a player that are on map's global leaderboard")]
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
#[example("badewanne3 -dt! -a 97.5..99.5 -r 42 --p --asc")]
#[example("vaxei +hdhr -r 1..5 --r")]
#[aliases("osgc")]
pub async fn osustatsglobalsctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::CTB, ctx, msg, args).await
}

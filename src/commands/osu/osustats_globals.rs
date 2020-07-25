use crate::{
    arguments::{Args, ModSelection, OsuStatsArgs},
    embeds::{EmbedData, OsuStatsGlobalsEmbed},
    pagination::{OsuStatsGlobalsPagination, Pagination},
    scraper::{OsuStatsScore, Scraper},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt,
    },
    BotResult, Context,
};

use rosu::{backend::requests::UserRequest, models::GameMode};
use std::{collections::BTreeMap, fmt::Write, sync::Arc};
use twilight::model::channel::Message;

async fn osustats_send(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    let name = {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        links.get(msg.author.id.as_u64()).cloned()
    };
    let args = match OsuStatsArgs::new(args, name, mode) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.respond(&ctx, err_msg).await?;
            return Ok(());
        }
    };
    let params = args.params;
    let user = {
        let req = UserRequest::with_username(&params.username).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match req.queue_single(osu).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                let content = format!("User `{}` was not found", params.username);
                msg.respond(&ctx, content).await?;
                return Ok(());
            }
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };
    let (scores, amount) = {
        let data = ctx.data.read().await;
        let scraper = data.get::<Scraper>().unwrap();
        match scraper.get_global_scores(&params).await {
            Ok((scores, amount)) => (
                scores
                    .into_iter()
                    .enumerate()
                    .collect::<BTreeMap<usize, OsuStatsScore>>(),
                amount,
            ),
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, amount);
    let data = match OsuStatsGlobalsEmbed::new(&user, &scores, amount, (1, pages), ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why.into());
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
    if let Some((mods, selection)) = params.mods {
        let _ = write!(
            content,
            " ~ `Mods: {}{}`",
            match selection {
                ModSelection::Exact => "",
                ModSelection::Excludes => "Exclude ",
                ModSelection::Includes | ModSelection::None => "Include ",
            },
            mods
        );
    }

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(ctx, |m| m.content(content).embed(|e| data.build(e)))
        .await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        resp.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination =
        OsuStatsGlobalsPagination::new(ctx, resp, msg.author.id, user, scores, amount, params)
            .await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
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
    osustats_send(GameMode::STD, ctx, msg, args).await
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
    osustats_send(GameMode::MNA, ctx, msg, args).await
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
    osustats_send(GameMode::TKO, ctx, msg, args).await
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
    osustats_send(GameMode::CTB, ctx, msg, args).await
}

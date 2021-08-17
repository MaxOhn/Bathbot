use super::{ErrorType, GradeArg};
use crate::{
    embeds::{EmbedData, TopEmbed, TopSingleEmbed},
    pagination::{Pagination, TopPagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, numbers,
        osu::ModSelection,
        CowUtils, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use futures::future::TryFutureExt;
use rosu_v2::prelude::{
    GameMode, Grade, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score, User,
};
use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    sync::Arc,
};
use tokio::time::{sleep, Duration};
use twilight_model::application::interaction::application_command::CommandDataOption;

pub(super) async fn _top(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut args: TopArgs,
) -> BotResult<()> {
    if args.index.filter(|n| *n > 100).is_some() {
        let content = "Can't have more than 100 top scores.";

        return data.error(&ctx, content).await;
    }

    let mode = args.mode;

    if args.sort_by == TopOrder::Position && args.has_dash_r {
        let mode_long = mode_long(mode);
        let prefix = ctx.config_first_prefix(data.guild_id());

        let mode_short = match mode {
            GameMode::STD => "",
            GameMode::MNA => "m",
            GameMode::TKO => "t",
            GameMode::CTB => "c",
        };

        let content = format!(
            "`{prefix}top{mode_long} -r`? I think you meant `{prefix}recentbest{mode_long}` \
            or `{prefix}rb{mode_short}` for short ;)",
            mode_long = mode_long,
            mode_short = mode_short,
            prefix = prefix
        );

        return data.error(&ctx, content).await;
    } else if args.has_dash_p_or_i {
        let cmd = match args.sort_by {
            TopOrder::Date => "rb",
            TopOrder::Position => "top",
            _ => unreachable!(),
        };

        let mode_long = mode_long(mode);
        let prefix = ctx.config_first_prefix(data.guild_id());

        let content = format!(
            "`{prefix}{cmd}{mode} -i / -p`? \
            Try putting the number right after the command, e.g. `{prefix}{cmd}{mode}42`, or use the arrow reactions.",
            mode = mode_long,
            cmd = cmd,
            prefix = prefix
        );

        return data.error(&ctx, content).await;
    }

    let name = match args.name.take() {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    // Retrieve the user and their top scores
    let user_fut = super::request_user(&ctx, &name, Some(mode)).map_err(From::from);

    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let scores_fut = super::prepare_scores(&ctx, scores_fut);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => (user, scores),
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    // Filter scores according to mods, combo, acc, and grade
    let scores = filter_scores(scores, &args);

    if args.index.filter(|n| *n > scores.len()).is_some() {
        let content = format!(
            "`{}` only has {} top scores with the specified properties",
            name,
            scores.len()
        );

        return data.error(&ctx, content).await;
    }

    // Add maps of scores to DB
    let scores_iter = scores.iter().map(|(_, score)| score);

    if let Err(why) = ctx.psql().store_scores_maps(scores_iter).await {
        unwind_error!(warn, why, "Error while adding score maps to DB: {}")
    }

    if let Some(num) = args.index {
        single_embed(ctx, data, user, scores, num.saturating_sub(1)).await?;
    } else {
        let content = match args.sort_by {
            TopOrder::Date => Some(format!("Most recent scores in `{}`'s top100:", name)),
            _ => {
                let cond = args.mods.is_some()
                    || args.acc_min.is_some()
                    || args.combo_min.is_some()
                    || args.grade.is_some();

                if cond {
                    let amount = scores.len();

                    let content = format!(
                        "Found {num} top score{plural} with the specified properties:",
                        num = amount,
                        plural = if amount != 1 { "s" } else { "" }
                    );

                    Some(content)
                } else {
                    None
                }
            }
        };

        paginated_embed(ctx, data, user, scores, content).await?;
    }

    Ok(())
}

#[command]
#[short_desc("Display a user's top plays")]
#[long_desc(
    "Display a user's top plays.\n\
     Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
     There are also multiple options you can set by specifying `key=value`.\n\
     These are the keys with their values:\n\
     - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
     - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
     - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
     - `sort`: `acc`, `combo`, `date` (= `rb` command), `length`, or `position` (default)\n\
     - `reverse`: `true` or `false` (default)\n\
     \n\
     Instead of showing the scores in a list, you can also __show a single score__ by \
     specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
    [grade=grade[..grade]] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("topscores", "osutop")]
async fn top(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::STD, num) {
                Ok(top_args) => _top(ctx, CommandData::Message { msg, args, num }, top_args).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

#[command]
#[short_desc("Display a user's top mania plays")]
#[long_desc(
    "Display a user's top mania plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
    - `sort`: `acc`, `combo`, `date` (= `rb` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
   [grade=grade[..grade]] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("topm")]
async fn topmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::MNA, num) {
                Ok(top_args) => _top(ctx, CommandData::Message { msg, args, num }, top_args).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

#[command]
#[short_desc("Display a user's top taiko plays")]
#[long_desc(
    "Display a user's top taiko plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
    - `sort`: `acc`, `combo`, `date` (= `rb` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
   [grade=grade[..grade]] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("topt")]
async fn toptaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::TKO, num) {
                Ok(top_args) => _top(ctx, CommandData::Message { msg, args, num }, top_args).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

#[command]
#[short_desc("Display a user's top ctb plays")]
#[long_desc(
    "Display a user's top ctb plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
    - `sort`: `acc`, `combo`, `date` (= `rb` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
   [grade=grade[..grade]] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("topc")]
async fn topctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::CTB, num) {
                Ok(top_args) => _top(ctx, CommandData::Message { msg, args, num }, top_args).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

#[command]
#[short_desc("Sort a user's top plays by date")]
#[long_desc(
    "Display a user's most recent top plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
   "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=grade[..grade]] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("rb")]
async fn recentbest(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::STD, num) {
                Ok(mut top_args) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;

                    _top(ctx, data, top_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

#[command]
#[short_desc("Sort a user's top mania plays by date")]
#[long_desc(
    "Display a user's most recent top mania plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
   "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=grade[..grade]] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("rbm")]
async fn recentbestmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::MNA, num) {
                Ok(mut top_args) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;

                    _top(ctx, data, top_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

#[command]
#[short_desc("Sort a user's top taiko plays by date")]
#[long_desc(
    "Display a user's most recent top taiko plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
   "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=grade[..grade]] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("rbt")]
async fn recentbesttaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::TKO, num) {
                Ok(mut top_args) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;

                    _top(ctx, data, top_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

#[command]
#[short_desc("Sort a user's top ctb plays by date")]
#[long_desc(
    "Display a user's most recent top ctb plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: single grade or two grades of the form `a..b` e.g. `grade=b..sh`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
   "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=grade[..grade]] [reverse=true/false]"
)]
#[example(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B..S reverse=true"
)]
#[aliases("rbc")]
async fn recentbestctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TopArgs::args(&ctx, &mut args, GameMode::CTB, num) {
                Ok(mut top_args) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;

                    _top(ctx, data, top_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, command).await,
    }
}

fn filter_scores(scores: Vec<Score>, args: &TopArgs) -> Vec<(usize, Score)> {
    let selection = args.mods;
    let grade = args.grade;

    let mut scores_indices: Vec<(usize, Score)> = scores
        .into_iter()
        .enumerate()
        .filter(|(_, s)| {
            match grade {
                Some(GradeArg::Single(grade)) => {
                    if !s.grade.eq_letter(grade) {
                        return false;
                    }
                }
                Some(GradeArg::Range { bot, top }) => {
                    if s.grade < bot || s.grade > top {
                        return false;
                    }
                }
                None => {}
            }

            let mod_bool = match selection {
                None => true,
                Some(ModSelection::Exact(mods)) => {
                    if mods.is_empty() {
                        s.mods.is_empty()
                    } else {
                        mods == s.mods
                    }
                }
                Some(ModSelection::Include(mods)) => {
                    if mods.is_empty() {
                        s.mods.is_empty()
                    } else {
                        s.mods.contains(mods)
                    }
                }
                Some(ModSelection::Exclude(mods)) => {
                    if mods.is_empty() && s.mods.is_empty() {
                        false
                    } else if mods.is_empty() {
                        true
                    } else {
                        !s.mods.contains(mods)
                    }
                }
            };

            if !mod_bool {
                return false;
            }

            let acc = s.accuracy;
            let acc_bool = match (args.acc_min, args.acc_max) {
                (Some(a), _) if a > acc => false,
                (_, Some(a)) if a < acc => false,
                _ => true,
            };

            let combo_bool = match (args.combo_min, args.combo_max) {
                (Some(c), _) if c > s.max_combo => false,
                (_, Some(c)) if c < s.max_combo => false,
                _ => true,
            };

            acc_bool && combo_bool
        })
        .collect();

    match args.sort_by {
        TopOrder::Acc => {
            scores_indices.sort_unstable_by(|(_, a), (_, b)| {
                b.accuracy
                    .partial_cmp(&a.accuracy)
                    .unwrap_or(Ordering::Equal)
            });
        }
        TopOrder::Combo => scores_indices.sort_unstable_by_key(|(_, s)| Reverse(s.max_combo)),
        TopOrder::Date => scores_indices.sort_unstable_by_key(|(_, s)| Reverse(s.created_at)),
        TopOrder::Length => scores_indices.sort_unstable_by_key(|(_, s)| {
            s.map
                .as_ref()
                .map_or(Reverse(0), |map| Reverse(map.seconds_drain))
        }),
        TopOrder::Position => {}
    }

    if args.reverse {
        scores_indices.reverse();
    }

    scores_indices.iter_mut().for_each(|(i, _)| *i += 1);

    scores_indices
}

fn mode_long(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::MNA => "mania",
        GameMode::TKO => "taiko",
        GameMode::CTB => "ctb",
    }
}

async fn single_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    scores: Vec<(usize, Score)>,
    idx: usize,
) -> BotResult<()> {
    let (idx, score) = scores.get(idx).unwrap();
    let map = score.map.as_ref().unwrap();

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let globals = match map.status {
        Ranked | Loved | Qualified | Approved => {
            // TODO: Add .limit(50) when supported by osu!api
            match ctx.osu().beatmap_scores(map.map_id).await {
                Ok(scores) => Some(scores),
                Err(why) => {
                    unwind_error!(warn, why, "Error while getting global scores: {}");

                    None
                }
            }
        }
        _ => None,
    };

    let embed_data = TopSingleEmbed::new(&user, score, *idx, globals.as_deref()).await?;

    // Creating the embed
    let builder = embed_data.as_builder().build().into();
    let response_raw = data.create_message(&ctx, builder).await?;

    // TODO
    // ctx.store_msg(response.id);

    let data = data.compact();

    // Minimize embed after delay
    tokio::spawn(async move {
        sleep(Duration::from_secs(45)).await;

        // TODO
        // if !ctx.remove_msg(response.id) {
        //     return;
        // }

        let builder = embed_data.into_builder().build().into();

        if let Err(why) = data.update_message(&ctx, builder, response_raw).await {
            unwind_error!(warn, why, "Error minimizing top msg: {}");
        }
    });

    Ok(())
}

async fn paginated_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    scores: Vec<(usize, Score)>,
    content: Option<String>,
) -> BotResult<()> {
    let pages = numbers::div_euclid(5, scores.len());
    let embed_data = TopEmbed::new(&user, scores.iter().take(5), (1, pages)).await;
    let embed = embed_data.into_builder().build();

    // Creating the embed
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(content) = content {
        builder = builder.content(content);
    }

    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        return Ok(());
    }

    let response = data.get_response(&ctx, response_raw).await?;

    // Pagination
    let pagination = TopPagination::new(response, user, scores);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (top): {}")
        }
    });

    Ok(())
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TopOrder {
    Acc,
    Combo,
    Date,
    Length,
    Position,
}

impl Default for TopOrder {
    fn default() -> Self {
        Self::Position
    }
}

pub(super) struct TopArgs {
    name: Option<Name>,
    mods: Option<ModSelection>,
    acc_min: Option<f32>,
    acc_max: Option<f32>,
    combo_min: Option<u32>,
    combo_max: Option<u32>,
    grade: Option<GradeArg>,
    sort_by: TopOrder,
    reverse: bool,
    mode: GameMode,
    index: Option<usize>,
    has_dash_r: bool,
    has_dash_p_or_i: bool,
}

impl TopArgs {
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want included mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

    const ERR_PARSE_ACC: &'static str = "Failed to parse `accuracy`.\n\
        Must be either decimal number \
        or two decimal numbers of the form `a..b` e.g. `97.5..98.5`.";

    const ERR_PARSE_COMBO: &'static str = "Failed to parse `combo`.\n\
        Must be either a positive integer \
        or two positive integers of the form `a..b` e.g. `501..1234`.";

    const ERR_PARSE_GRADE: &'static str = "Failed to parse `grade`.\n\
        Must be either a single grade or two grades of the form `a..b` e.g. `C..S`.\n\
        Valid grades are: `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, or `D`";

    fn args(
        ctx: &Context,
        args: &mut Args,
        mode: GameMode,
        index: Option<usize>,
    ) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut mods = None;
        let mut acc_min = None;
        let mut acc_max = None;
        let mut combo_min = None;
        let mut combo_max = None;
        let mut grade = None;
        let mut sort_by = None;
        let mut reverse = None;
        let mut has_dash_r = None;
        let mut has_dash_p_or_i = None;

        for arg in args.map(CowUtils::cow_to_ascii_lowercase) {
            if arg.as_ref() == "-r" {
                has_dash_r = Some(true);
            } else if matches!(arg.as_ref(), "-p" | "-i") {
                has_dash_p_or_i = Some(true);
            } else if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "acc" | "accuracy" | "a" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                0.0
                            } else if let Ok(num) = bot.parse::<f32>() {
                                num.max(0.0).min(100.0)
                            } else {
                                return Err(Self::ERR_PARSE_ACC.into());
                            };

                            let max = if top.is_empty() {
                                100.0
                            } else if let Ok(num) = top.parse::<f32>() {
                                num.max(0.0).min(100.0)
                            } else {
                                return Err(Self::ERR_PARSE_ACC.into());
                            };

                            acc_min = Some(min.min(max));
                            acc_max = Some(min.max(max));
                        }
                        None => acc_min = Some(value.parse().map_err(|_| Self::ERR_PARSE_ACC)?),
                    },
                    "combo" | "c" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                0
                            } else if let Ok(num) = bot.parse() {
                                num
                            } else {
                                return Err(Self::ERR_PARSE_COMBO.into());
                            };

                            let max = top.parse().map_err(|_| Self::ERR_PARSE_COMBO)?;

                            combo_min = Some(min.min(max));
                            combo_max = Some(min.max(max));
                        }
                        None => combo_min = Some(value.parse().map_err(|_| Self::ERR_PARSE_COMBO)?),
                    },
                    "grade" | "g" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                Grade::XH
                            } else if let Some(grade) = parse_grade(bot) {
                                grade
                            } else {
                                return Err(Self::ERR_PARSE_GRADE.into());
                            };

                            let max = if top.is_empty() {
                                Grade::D
                            } else if let Some(grade) = parse_grade(top) {
                                grade
                            } else {
                                return Err(Self::ERR_PARSE_GRADE.into());
                            };

                            let bot = if min < max { min } else { max };
                            let top = if min > max { min } else { max };

                            grade = Some(GradeArg::Range { bot, top })
                        }
                        None => match parse_grade(value).map(GradeArg::Single) {
                            Some(grade_) => grade = Some(grade_),
                            None => return Err(Self::ERR_PARSE_GRADE.into()),
                        },
                    },
                    "sort" | "s" | "order" | "ordering" => match value {
                        "acc" | "a" | "accuracy" => sort_by = Some(TopOrder::Acc),
                        "combo" | "c" => sort_by = Some(TopOrder::Combo),
                        "date" | "d" | "recent" | "r" => sort_by = Some(TopOrder::Date),
                        "length" | "len" | "l" => sort_by = Some(TopOrder::Length),
                        "position" | "p" => sort_by = Some(TopOrder::Position),
                        _ => {
                            let content = "Failed to parse `sort`.\n\
                            Must be either `acc`, `combo`, `date`, `length`, or `position`";

                            return Err(content.into());
                        }
                    },
                    "mods" => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Err(Self::ERR_PARSE_MODS.into()),
                    },
                    "reverse" | "r" => match value {
                        "true" | "1" => reverse = Some(true),
                        "false" | "0" => reverse = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `reverse`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `acc`, `combo`, `sort`, `grade`, or `reverse`.",
                            key
                        );

                        return Err(content.into());
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(arg.as_ref()) {
                mods = Some(mods_);
            } else {
                name = Some(Args::try_link_name(ctx, arg.as_ref())?);
            }
        }

        let args = Self {
            name,
            mods,
            acc_min,
            acc_max,
            combo_min,
            combo_max,
            grade,
            sort_by: sort_by.unwrap_or_default(),
            reverse: reverse.unwrap_or(false),
            mode,
            index,
            has_dash_r: has_dash_r.unwrap_or(false),
            has_dash_p_or_i: has_dash_p_or_i.unwrap_or(false),
        };

        Ok(args)
    }

    pub(super) fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut username = None;
        let mut mode = None;
        let mut mods = None;
        let mut grade = None;
        let mut order = None;
        let mut reverse = None;
        let mut index = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!(ctx, value, "top current"),
                    "mode" => mode = parse_mode_option!(value, "top current"),
                    "mods" => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
                    "sort" => match value.as_str() {
                        "acc" => order = Some(TopOrder::Acc),
                        "combo" => order = Some(TopOrder::Combo),
                        "date" => order = Some(TopOrder::Date),
                        "len" => order = Some(TopOrder::Length),
                        "pos" => order = Some(TopOrder::Position),
                        _ => bail_cmd_option!("top current sort", string, value),
                    },
                    "grade" => match value.as_str() {
                        "SS" => {
                            grade = Some(GradeArg::Range {
                                bot: Grade::X,
                                top: Grade::XH,
                            })
                        }
                        "S" => {
                            grade = Some(GradeArg::Range {
                                bot: Grade::S,
                                top: Grade::SH,
                            })
                        }
                        "A" => grade = Some(GradeArg::Single(Grade::A)),
                        "B" => grade = Some(GradeArg::Single(Grade::B)),
                        "C" => grade = Some(GradeArg::Single(Grade::C)),
                        "D" => grade = Some(GradeArg::Single(Grade::D)),
                        _ => bail_cmd_option!("top current grade", string, value),
                    },
                    _ => bail_cmd_option!("top current", string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    "index" => index = Some(value.max(0) as usize),
                    _ => bail_cmd_option!("top current", integer, name),
                },
                CommandDataOption::Boolean { name, value } => match name.as_str() {
                    "reverse" => reverse = Some(value),
                    _ => bail_cmd_option!("top current", boolean, name),
                },
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("top current", subcommand, name)
                }
            }
        }

        let args = Self {
            name: username,
            mods,
            acc_min: None,
            acc_max: None,
            combo_min: None,
            combo_max: None,
            grade,
            sort_by: order.unwrap_or_default(),
            reverse: reverse.unwrap_or(false),
            mode: mode.unwrap_or(GameMode::STD),
            index,
            has_dash_r: false,
            has_dash_p_or_i: false,
        };

        Ok(Ok(args))
    }
}

fn parse_grade(arg: &str) -> Option<Grade> {
    match arg {
        "xh" | "ssh" => Some(Grade::XH),
        "ss" | "x" => Some(Grade::X),
        "sh" => Some(Grade::SH),
        "s" => Some(Grade::S),
        "a" => Some(Grade::A),
        "b" => Some(Grade::B),
        "c" => Some(Grade::C),
        "d" => Some(Grade::D),
        _ => None,
    }
}

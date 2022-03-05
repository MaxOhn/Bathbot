use std::{
    cmp::{Ordering, Reverse},
    fmt::Write,
    mem,
    sync::Arc,
};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{
    Beatmap, BeatmapsetCompact, GameMode, GameMods, Grade, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score, User,
};
use tokio::time::{sleep, Duration};
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            ApplicationCommand,
        },
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        parse_discord, parse_mode_option, DoubleResultCow, MyCommand, MyCommandOption,
    },
    database::UserConfig,
    embeds::{EmbedData, TopEmbed, TopSingleEmbed},
    error::Error,
    pagination::{Pagination, TopPagination},
    pp::PpCalculator,
    tracking::process_osu_tracking,
    util::{
        constants::{
            common_literals::{
                ACC, ACCURACY, COMBO, CONSIDER_GRADE, CTB, DISCORD, GRADE, INDEX, MANIA, MODE,
                MODS, NAME, REVERSE, SORT, TAIKO,
            },
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher, numbers,
        osu::ModSelection,
        ApplicationCommandExt, CowUtils, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use super::{option_discord, option_mode, option_mods_explicit, option_name, GradeArg};

pub async fn _top(ctx: Arc<Context>, data: CommandData<'_>, args: TopArgs) -> BotResult<()> {
    if args.index.filter(|n| *n > 100).is_some() {
        let content = "Can't have more than 100 top scores.";

        return data.error(&ctx, content).await;
    }

    let mode = args.config.mode.unwrap_or(GameMode::STD);

    if args.sort_by == TopOrder::Pp && args.has_dash_r {
        let mode_long = mode_long(mode);
        let prefix = ctx.guild_first_prefix(data.guild_id()).await;

        let mode_short = match mode {
            GameMode::STD => "",
            GameMode::MNA => "m",
            GameMode::TKO => "t",
            GameMode::CTB => "c",
        };

        let content = format!(
            "`{prefix}top{mode_long} -r`? I think you meant `{prefix}recentbest{mode_long}` \
            or `{prefix}rb{mode_short}` for short ;)",
        );

        return data.error(&ctx, content).await;
    } else if args.has_dash_p_or_i {
        let cmd = match args.sort_by {
            TopOrder::Date => "rb",
            TopOrder::Pp => "top",
            _ => unreachable!(),
        };

        let mode_long = mode_long(mode);
        let prefix = ctx.guild_first_prefix(data.guild_id()).await;

        let content = format!(
            "`{prefix}{cmd}{mode_long} -i / -p`? \
            Try putting the number right after the command, e.g. `{prefix}{cmd}{mode_long}42`, or use the arrow reactions.",
        );

        return data.error(&ctx, content).await;
    }

    let name = match args.config.username() {
        Some(name) => name.as_str(),
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name, mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    // Filter scores according to mods, combo, acc, and grade
    let scores = filter_scores(&ctx, scores, &args).await;

    if args.index.filter(|n| *n > scores.len()).is_some() {
        let content = format!(
            "`{name}` only has {} top scores with the specified properties",
            scores.len()
        );

        return data.error(&ctx, content).await;
    }

    // Add maps of scores to DB
    let scores_iter = scores.iter().map(|(_, score)| score);

    // Store maps of scores in DB; combo was inserted earlier
    if let Err(err) = ctx.psql().store_scores_maps(scores_iter).await {
        warn!("{:?}", Report::new(err));
    }

    match (args.index, scores.len()) {
        (Some(num), _) => {
            let maximize = match (args.config.embeds_maximized, data.guild_id()) {
                (Some(embeds_maximized), _) => embeds_maximized,
                (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
                (None, None) => true,
            };

            let num = num.saturating_sub(1);
            single_embed(ctx, data, user, scores, num, maximize, None).await?;
        }
        (_, 1) => {
            let maximize = match (args.config.embeds_maximized, data.guild_id()) {
                (Some(embeds_maximized), _) => embeds_maximized,
                (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
                (None, None) => true,
            };

            let content = write_content(name, &args, 1);
            single_embed(ctx, data, user, scores, 0, maximize, content).await?;
        }
        (None, _) => {
            let content = write_content(name, &args, scores.len());
            paginated_embed(ctx, data, user, scores, args.sort_by, content).await?;
        }
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    top_args.config.mode.get_or_insert(GameMode::STD);

                    _top(ctx, CommandData::Message { msg, args, num }, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
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
    - `sort`: `acc`, `combo`, `date` (= `rbm` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<topm2 badewanne3`."
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    top_args.config.mode = Some(GameMode::MNA);

                    _top(ctx, CommandData::Message { msg, args, num }, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
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
    - `sort`: `acc`, `combo`, `date` (= `rbt` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<topt2 badewanne3`."
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    top_args.config.mode = Some(GameMode::TKO);

                    _top(ctx, CommandData::Message { msg, args, num }, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
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
    - `sort`: `acc`, `combo`, `date` (= `rbc` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<topc2 badewanne3`."
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    top_args.config.mode = Some(GameMode::CTB);

                    _top(ctx, CommandData::Message { msg, args, num }, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
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
    specifying a number right after the command, e.g. `<rb2 badewanne3`."
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;
                    top_args.config.mode.get_or_insert(GameMode::STD);

                    _top(ctx, data, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
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
    specifying a number right after the command, e.g. `<rbm2 badewanne3`."
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;
                    top_args.config.mode = Some(GameMode::MNA);

                    _top(ctx, data, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
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
    specifying a number right after the command, e.g. `<rbt2 badewanne3`."
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;
                    top_args.config.mode = Some(GameMode::TKO);

                    _top(ctx, data, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
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
    specifying a number right after the command, e.g. `<rbc2 badewanne3`."
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
            match TopArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut top_args)) => {
                    let data = CommandData::Message { msg, args, num };
                    top_args.sort_by = TopOrder::Date;
                    top_args.config.mode = Some(GameMode::CTB);

                    _top(ctx, data, top_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

async fn filter_scores(ctx: &Context, scores: Vec<Score>, args: &TopArgs) -> Vec<(usize, Score)> {
    let selection = args.mods;
    let grade = args.grade;

    let mut scores_indices: Vec<(usize, Score)> = scores
        .into_iter()
        .enumerate()
        .filter(|(_, s)| {
            if let Some(perfect_combo) = args.perfect_combo {
                let map_combo = match s.map.as_ref().and_then(|m| m.max_combo) {
                    Some(combo) => combo,
                    None => return false,
                };

                if perfect_combo ^ (map_combo == s.max_combo) {
                    return false;
                }
            }

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

    if let Some(query) = args.query.as_deref() {
        let needle = query.cow_to_ascii_lowercase();
        let mut haystack = String::new();

        scores_indices.retain(|(_, score)| {
            let Beatmap { version, .. } = score.map.as_ref().unwrap();
            let BeatmapsetCompact { artist, title, .. } = score.mapset.as_ref().unwrap();
            haystack.clear();

            let _ = write!(
                haystack,
                "{} - {} [{}]",
                artist.cow_to_ascii_lowercase(),
                title.cow_to_ascii_lowercase(),
                version.cow_to_ascii_lowercase()
            );

            haystack.contains(needle.as_ref())
        });
    }

    args.sort_by.apply(ctx, &mut scores_indices).await;

    if args.reverse {
        scores_indices.reverse();
    }

    scores_indices
}

fn mode_long(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::MNA => MANIA,
        GameMode::TKO => TAIKO,
        GameMode::CTB => CTB,
    }
}

async fn single_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    scores: Vec<(usize, Score)>,
    idx: usize,
    maximize: bool,
    content: Option<String>,
) -> BotResult<()> {
    let (idx, score) = scores.get(idx).unwrap();
    let map = score.map.as_ref().unwrap();

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let global_idx = match map.status {
        Ranked | Loved | Qualified | Approved => {
            // TODO: Add .limit(50) when supported by osu!api
            match ctx.osu().beatmap_scores(map.map_id).await {
                Ok(scores) => scores.iter().position(|s| s == score),
                Err(why) => {
                    let report = Report::new(why).wrap_err("failed to get global scores");
                    warn!("{report:?}");

                    None
                }
            }
        }
        _ => None,
    };

    let embed_data = TopSingleEmbed::new(&user, score, Some(*idx), global_idx, &ctx).await?;

    // Only maximize if config allows it
    if maximize {
        let mut builder = MessageBuilder::new().embed(embed_data.as_builder().build());

        if let Some(content) = content {
            builder = builder.content(content);
        }

        let response = data.create_message(&ctx, builder).await?.model().await?;

        ctx.store_msg(response.id);

        // Minimize embed after delay
        tokio::spawn(async move {
            sleep(Duration::from_secs(45)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let builder = embed_data.into_builder().build().into();

            if let Err(why) = response.update_message(&ctx, builder).await {
                let report = Report::new(why).wrap_err("failed to minimize top message");
                warn!("{:?}", report);
            }
        });
    } else {
        let mut builder = MessageBuilder::new().embed(embed_data.into_builder().build());

        if let Some(content) = content {
            builder = builder.content(content);
        }

        data.create_message(&ctx, builder).await?;
    }

    Ok(())
}

async fn paginated_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopOrder,
    content: Option<String>,
) -> BotResult<()> {
    let pages = numbers::div_euclid(5, scores.len());
    let embed_data = TopEmbed::new(&user, scores.iter().take(5), &ctx, sort_by, (1, pages)).await;
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

    let response = response_raw.model().await?;

    // Pagination
    let pagination = TopPagination::new(response, user, scores, sort_by, Arc::clone(&ctx));
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TopOrder {
    Acc,
    Bpm,
    Combo,
    Date,
    Length,
    Misses,
    Pp,
    Stars,
}

pub trait SortableScore {
    fn get(&self) -> &Score;
}

impl SortableScore for Score {
    #[inline]
    fn get(&self) -> &Score {
        self
    }
}

impl SortableScore for (usize, Score) {
    #[inline]
    fn get(&self) -> &Score {
        &self.1
    }
}

impl TopOrder {
    pub async fn apply<S: SortableScore>(self, ctx: &Context, scores: &mut [S]) {
        match self {
            Self::Acc => {
                scores.sort_unstable_by(|a, b| {
                    b.get()
                        .accuracy
                        .partial_cmp(&a.get().accuracy)
                        .unwrap_or(Ordering::Equal)
                });
            }
            Self::Bpm => scores.sort_unstable_by(|a, b| {
                let a = a.get();
                let b = b.get();

                fn clock_rate(mods: GameMods) -> f32 {
                    if mods.contains(GameMods::DoubleTime) {
                        1.5
                    } else if mods.contains(GameMods::HalfTime) {
                        0.75
                    } else {
                        1.0
                    }
                }

                let a_clock_rate = clock_rate(a.mods);
                let b_clock_rate = clock_rate(b.mods);

                let a_bpm = a.map.as_ref().map_or(0.0, |map| map.bpm * a_clock_rate);
                let b_bpm = b.map.as_ref().map_or(0.0, |map| map.bpm * b_clock_rate);

                b_bpm.partial_cmp(&a_bpm).unwrap_or(Ordering::Equal)
            }),
            Self::Combo => scores.sort_unstable_by_key(|s| Reverse(s.get().max_combo)),
            Self::Date => scores.sort_unstable_by_key(|s| Reverse(s.get().created_at)),
            Self::Length => scores.sort_unstable_by_key(|s| {
                Reverse(s.get().map.as_ref().map_or(0, |map| map.seconds_drain))
            }),
            Self::Misses => scores.sort_unstable_by(|a, b| {
                let a = a.get();
                let b = b.get();

                b.statistics
                    .count_miss
                    .cmp(&a.statistics.count_miss)
                    .then_with(|| {
                        let hits_a = a.total_hits();
                        let hits_b = b.total_hits();

                        let ratio_a = a.statistics.count_miss as f32 / hits_a as f32;
                        let ratio_b = b.statistics.count_miss as f32 / hits_b as f32;

                        ratio_b
                            .partial_cmp(&ratio_a)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| hits_b.cmp(&hits_a))
                    })
            }),
            Self::Pp => scores.sort_unstable_by(|a, b| {
                b.get()
                    .pp
                    .partial_cmp(&a.get().pp)
                    .unwrap_or(Ordering::Equal)
            }),
            Self::Stars => {
                let mut stars = HashMap::new();

                for score in scores.iter().map(SortableScore::get) {
                    let id = score.score_id;

                    let map = match score.map.as_ref() {
                        Some(map) => map,
                        None => continue,
                    };

                    if !score.mods.changes_stars(score.mode) {
                        stars.insert(id, map.stars);

                        continue;
                    }

                    let stars_ = match PpCalculator::new(ctx, map.map_id).await {
                        Ok(mut calc) => calc.mods(score.mods).stars() as f32,
                        Err(err) => {
                            warn!("{:?}", Report::new(err));

                            continue;
                        }
                    };

                    stars.insert(id, stars_);
                }

                scores.sort_unstable_by(|a, b| {
                    let stars_a = stars.get(&a.get().score_id).unwrap_or(&0.0);
                    let stars_b = stars.get(&b.get().score_id).unwrap_or(&0.0);

                    stars_b.partial_cmp(stars_a).unwrap_or(Ordering::Equal)
                })
            }
        }
    }
}

impl Default for TopOrder {
    fn default() -> Self {
        Self::Pp
    }
}

pub async fn slash_top(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let options = command.yoink_options();

    match TopArgs::slash(&ctx, &command, options).await? {
        Ok(args) => _top(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub struct TopArgs {
    config: UserConfig,
    mods: Option<ModSelection>,
    acc_min: Option<f32>,
    acc_max: Option<f32>,
    combo_min: Option<u32>,
    combo_max: Option<u32>,
    grade: Option<GradeArg>,
    pub sort_by: TopOrder,
    reverse: bool,
    perfect_combo: Option<bool>,
    index: Option<usize>,
    query: Option<String>,
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

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
        index: Option<usize>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
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
                    ACC | ACCURACY | "a" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let mut min = if bot.is_empty() {
                                0.0
                            } else if let Ok(num) = bot.parse::<f32>() {
                                num.max(0.0).min(100.0)
                            } else {
                                return Ok(Err(Self::ERR_PARSE_ACC.into()));
                            };

                            let mut max = if top.is_empty() {
                                100.0
                            } else if let Ok(num) = top.parse::<f32>() {
                                num.max(0.0).min(100.0)
                            } else {
                                return Ok(Err(Self::ERR_PARSE_ACC.into()));
                            };

                            if min > max {
                                mem::swap(&mut min, &mut max);
                            }

                            acc_min = Some(min);
                            acc_max = Some(max);
                        }
                        None => match value.parse() {
                            Ok(num) => acc_min = Some(num),
                            Err(_) => return Ok(Err(Self::ERR_PARSE_ACC.into())),
                        },
                    },
                    COMBO | "c" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let mut min = if bot.is_empty() {
                                0
                            } else if let Ok(num) = bot.parse() {
                                num
                            } else {
                                return Ok(Err(Self::ERR_PARSE_COMBO.into()));
                            };

                            let mut max = top.parse().ok();

                            if let Some(ref mut max) = max {
                                if min > *max {
                                    mem::swap(&mut min, max);
                                }
                            }

                            combo_min = Some(min);
                            combo_max = max;
                        }
                        None => match value.parse() {
                            Ok(num) => combo_min = Some(num),
                            Err(_) => return Ok(Err(Self::ERR_PARSE_COMBO.into())),
                        },
                    },
                    GRADE | "g" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let mut bot = if bot.is_empty() {
                                Grade::D
                            } else if let Some(grade) = parse_grade(bot) {
                                grade
                            } else {
                                return Ok(Err(Self::ERR_PARSE_GRADE.into()));
                            };

                            let mut top = if top.is_empty() {
                                Grade::XH
                            } else if let Some(grade) = parse_grade(top) {
                                grade
                            } else {
                                return Ok(Err(Self::ERR_PARSE_GRADE.into()));
                            };

                            if bot > top {
                                mem::swap(&mut bot, &mut top);
                            }

                            grade = Some(GradeArg::Range { bot, top })
                        }
                        None => match parse_grade(value).map(GradeArg::Single) {
                            Some(grade_) => grade = Some(grade_),
                            None => return Ok(Err(Self::ERR_PARSE_GRADE.into())),
                        },
                    },
                    SORT | "s" | "order" | "ordering" => match value {
                        ACC | "a" | ACCURACY => sort_by = Some(TopOrder::Acc),
                        COMBO | "c" => sort_by = Some(TopOrder::Combo),
                        "date" | "d" | "recent" | "r" => sort_by = Some(TopOrder::Date),
                        "length" | "len" | "l" => sort_by = Some(TopOrder::Length),
                        "pp" | "p" => sort_by = Some(TopOrder::Pp),
                        _ => {
                            let content = "Failed to parse `sort`.\n\
                            Must be either `acc`, `combo`, `date`, `length`, or `pp`";

                            return Ok(Err(content.into()));
                        }
                    },
                    MODS => match matcher::get_mods(value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
                    REVERSE | "r" => match value {
                        "true" | "t" | "1" => reverse = Some(true),
                        "false" | "f" | "0" => reverse = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `reverse`. Must be either `true` or `false`.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `acc`, `combo`, `sort`, `grade`, or `reverse`."
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(arg.as_ref()) {
                mods = Some(mods_);
            } else {
                match check_user_mention(ctx, arg.as_ref()).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        let args = Self {
            config,
            mods,
            acc_min,
            acc_max,
            combo_min,
            combo_max,
            grade,
            sort_by: sort_by.unwrap_or_default(),
            reverse: reverse.unwrap_or(false),
            perfect_combo: None,
            index,
            query: None,
            has_dash_r: has_dash_r.unwrap_or(false),
            has_dash_p_or_i: has_dash_p_or_i.unwrap_or(false),
        };

        Ok(Ok(args))
    }

    pub async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut mods = None;
        let mut grade = None;
        let mut order = None;
        let mut reverse = None;
        let mut perfect_combo = None;
        let mut index = None;
        let mut query = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MODE => config.mode = parse_mode_option(&value),
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
                    SORT => match value.as_str() {
                        ACC => order = Some(TopOrder::Acc),
                        "bpm" => order = Some(TopOrder::Bpm),
                        COMBO => order = Some(TopOrder::Combo),
                        "date" => order = Some(TopOrder::Date),
                        "len" => order = Some(TopOrder::Length),
                        "miss" => order = Some(TopOrder::Misses),
                        "pp" => order = Some(TopOrder::Pp),
                        "stars" => order = Some(TopOrder::Stars),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "query" => query = Some(value),
                    GRADE => match value.as_str() {
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
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => {
                    let number = (option.name == INDEX)
                        .then(|| value)
                        .ok_or(Error::InvalidCommandOptions)?;

                    index = Some(number.max(0) as usize);
                }
                CommandOptionValue::Boolean(value) => match option.name.as_str() {
                    REVERSE => reverse = Some(value),
                    "perfect_combo" => perfect_combo = Some(value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => config.osu = Some(osu),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let args = Self {
            config,
            mods,
            acc_min: None,
            acc_max: None,
            combo_min: None,
            combo_max: None,
            grade,
            sort_by: order.unwrap_or_default(),
            reverse: reverse.unwrap_or(false),
            perfect_combo,
            index,
            query,
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

fn write_content(name: &str, args: &TopArgs, amount: usize) -> Option<String> {
    let condition = args.acc_min.is_some()
        || args.acc_max.is_some()
        || args.combo_min.is_some()
        || args.combo_max.is_some()
        || args.grade.is_some()
        || args.mods.is_some()
        || args.perfect_combo.is_some()
        || args.query.is_some();

    if condition {
        Some(content_with_condition(args, amount))
    } else {
        let genitive = if name.ends_with('s') { "" } else { "s" };
        let reverse = if args.reverse { "reversed " } else { "" };

        let content = match args.sort_by {
            TopOrder::Acc => format!("`{name}`'{genitive} top100 sorted by {reverse}accuracy:"),
            TopOrder::Bpm => format!("`{name}`'{genitive} top100 sorted by {reverse}BPM:"),
            TopOrder::Combo => format!("`{name}`'{genitive} top100 sorted by {reverse}combo:"),
            TopOrder::Date if args.reverse => {
                format!("Oldest scores in `{name}`'{genitive} top100:")
            }
            TopOrder::Date => format!("Most recent scores in `{name}`'{genitive} top100:"),
            TopOrder::Length => format!("`{name}`'{genitive} top100 sorted by {reverse}length:"),
            TopOrder::Misses => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}miss count:")
            }
            TopOrder::Pp if !args.reverse => return None,
            TopOrder::Pp => format!("`{name}`'{genitive} top100 sorted by reversed pp:"),
            TopOrder::Stars => format!("`{name}`'{genitive} top100 sorted by {reverse}stars:"),
        };

        Some(content)
    }
}

fn content_with_condition(args: &TopArgs, amount: usize) -> String {
    let mut content = String::with_capacity(64);

    match args.sort_by {
        TopOrder::Acc => content.push_str("`Order: Accuracy"),
        TopOrder::Bpm => content.push_str("`Order: BPM"),
        TopOrder::Combo => content.push_str("`Order: Combo"),
        TopOrder::Date => content.push_str("`Order: Date"),
        TopOrder::Length => content.push_str("`Order: Length"),
        TopOrder::Misses => content.push_str("`Order: Misscount`"),
        TopOrder::Pp => content.push_str("`Order: Pp"),
        TopOrder::Stars => content.push_str("`Order: Stars`"),
    }

    if args.reverse {
        content.push_str(" (reverse)`");
    } else {
        content.push('`');
    }

    match (args.acc_min, args.acc_max) {
        (None, None) => {}
        (None, Some(max)) => {
            let _ = write!(content, " ~ `Acc: 0% - {}%`", numbers::round(max));
        }
        (Some(min), None) => {
            let _ = write!(content, " ~ `Acc: {}% - 100%`", numbers::round(min));
        }
        (Some(min), Some(max)) => {
            let _ = write!(
                content,
                " ~ `Acc: {}% - {}%`",
                numbers::round(min),
                numbers::round(max)
            );
        }
    }

    match (args.combo_min, args.combo_max) {
        (None, None) => {}
        (None, Some(max)) => {
            let _ = write!(content, " ~ `Combo: 0 - {max}`");
        }
        (Some(min), None) => {
            let _ = write!(content, " ~ `Combo: {min} - ∞`");
        }
        (Some(min), Some(max)) => {
            let _ = write!(content, " ~ `Combo: {min} - {max}`");
        }
    }

    match args.grade {
        Some(GradeArg::Single(grade)) => {
            let _ = write!(content, " ~ `Grade: {grade}`");
        }
        Some(GradeArg::Range { bot, top }) => {
            let _ = write!(content, " ~ `Grade: {bot} - {top}`");
        }
        None => {}
    }

    if let Some(selection) = args.mods {
        let _ = write!(
            content,
            " ~ `Mods: {}`",
            match selection {
                ModSelection::Exact(mods) => mods.to_string(),
                ModSelection::Exclude(mods) => format!("Exclude {mods}"),
                ModSelection::Include(mods) => format!("Include {mods}"),
            },
        );
    }

    if let Some(perfect_combo) = args.perfect_combo {
        let _ = write!(content, " ~ `Perfect combo: {perfect_combo}`");
    }

    if let Some(query) = args.query.as_deref() {
        let _ = write!(content, " ~ `Query: {query}`");
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching top score{plural}:");

    content
}

pub fn define_top() -> MyCommand {
    let mode = option_mode();
    let name = option_name();

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "date".to_owned(),
            value: "date".to_owned(),
        },
        CommandOptionChoice::String {
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: COMBO.to_owned(),
            value: COMBO.to_owned(),
        },
        CommandOptionChoice::String {
            name: "stars".to_owned(),
            value: "stars".to_owned(),
        },
        CommandOptionChoice::String {
            name: "length".to_owned(),
            value: "len".to_owned(),
        },
        CommandOptionChoice::String {
            name: "misses".to_owned(),
            value: "miss".to_owned(),
        },
        CommandOptionChoice::String {
            name: "bpm".to_owned(),
            value: "bpm".to_owned(),
        },
    ];

    let sort = MyCommandOption::builder(SORT, "Choose how the scores should be ordered")
        .help("Choose how the scores should be ordered, defaults to `pp`.")
        .string(sort_choices, false);

    let mods = option_mods_explicit();

    let index = MyCommandOption::builder(INDEX, "Choose a specific score index between 1 and 100")
        .min_int(1)
        .max_int(100)
        .integer(Vec::new(), false);

    let discord = option_discord();

    let reverse =
        MyCommandOption::builder(REVERSE, "Reverse the resulting score list").boolean(false);

    let grade_choices = vec![
        CommandOptionChoice::String {
            name: "SS".to_owned(),
            value: "SS".to_owned(),
        },
        CommandOptionChoice::String {
            name: "S".to_owned(),
            value: "S".to_owned(),
        },
        CommandOptionChoice::String {
            name: "A".to_owned(),
            value: "A".to_owned(),
        },
        CommandOptionChoice::String {
            name: "B".to_owned(),
            value: "B".to_owned(),
        },
        CommandOptionChoice::String {
            name: "C".to_owned(),
            value: "C".to_owned(),
        },
        CommandOptionChoice::String {
            name: "D".to_owned(),
            value: "D".to_owned(),
        },
    ];

    let query_description = "Search for a specific artist, title, or difficulty name";

    let query_help = "Search for a specific artist, title, or difficulty name.\n\
        Filters out all scores for which `{artist} - {title} [{version}]` does not contain the query.";

    let query = MyCommandOption::builder("query", query_description)
        .help(query_help)
        .string(vec![], false);

    let grade = MyCommandOption::builder(GRADE, CONSIDER_GRADE).string(grade_choices, false);

    let perfect_combo_description = "Filter out all scores that don't have a perfect combo";

    let perfect_combo =
        MyCommandOption::builder("perfect_combo", perfect_combo_description).boolean(false);

    MyCommand::new("top", "Display the user's current top100").options(vec![
        mode,
        name,
        sort,
        mods,
        index,
        discord,
        reverse,
        query,
        grade,
        perfect_combo,
    ])
}

use super::{prepare_scores, request_user, ErrorType};
use crate::{
    arguments::NameModArgs,
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    pp::{Calculations, PPCalculator},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers,
        osu::ModSelection,
        MessageExt,
    },
    Args, BotResult, Context,
};

use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score};
use std::{cmp::Ordering, fmt::Write, sync::Arc};
use twilight_model::channel::Message;

const NM: GameMods = GameMods::NoMod;
const DT: GameMods = GameMods::DoubleTime;
const NC: GameMods = GameMods::NightCore;
const HT: GameMods = GameMods::HalfTime;
const EZ: GameMods = GameMods::Easy;
const HR: GameMods = GameMods::HardRock;
const PF: GameMods = GameMods::Perfect;
const SD: GameMods = GameMods::SuddenDeath;

async fn topif_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameModArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    if let Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) = args.mods {
        let mut content = None;
        let ezhr = EZ | HR;
        let dtht = DT | HT;

        if mods & ezhr == ezhr {
            content = Some("Looks like an invalid mod combination, EZ and HR exclude each other.");
        }

        if mods & dtht == dtht {
            content = Some("Looks like an invalid mod combination, DT and HT exclude each other");
        }

        if let Some(content) = content {
            return msg.error(&ctx, content).await;
        }
    }

    // Retrieve the user and their top scores
    let user_fut = request_user(&ctx, &name, Some(mode)).map_err(From::from);
    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let scores_fut = prepare_scores(&ctx, scores_fut);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => (user, scores),
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|s| s.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;
    let arg_mods = args.mods;

    // Modify scores
    let scores_fut = scores
        .into_iter()
        .enumerate()
        .map(|(i, mut score)| async move {
            let map = score.map.as_ref().unwrap();

            if map.convert {
                return Ok((i + 1, score, None));
            }

            let changed = match arg_mods {
                Some(ModSelection::Exact(mods)) => {
                    let changed = score.mods != mods;
                    score.mods = mods;

                    changed
                }
                Some(ModSelection::Exclude(mut mods)) if mods != NM => {
                    if mods.contains(DT) {
                        mods |= NC;
                    }

                    if mods.contains(SD) {
                        mods |= PF
                    }

                    let changed = score.mods.intersects(mods);
                    score.mods.remove(mods);

                    changed
                }
                Some(ModSelection::Include(mods)) if mods != NM => {
                    let mut changed = false;

                    if mods.contains(DT) && score.mods.contains(HT) {
                        score.mods.remove(HT);
                        changed = true;
                    }

                    if mods.contains(HT) && score.mods.contains(DT) {
                        score.mods.remove(NC);
                        changed = true;
                    }

                    if mods.contains(HR) && score.mods.contains(EZ) {
                        score.mods.remove(EZ);
                        changed = true;
                    }

                    if mods.contains(EZ) && score.mods.contains(HR) {
                        score.mods.remove(HR);
                        changed = true;
                    }

                    changed |= !score.mods.contains(mods);
                    score.mods.insert(mods);

                    changed
                }
                _ => false,
            };

            let mut calculations = Calculations::STARS | Calculations::MAX_PP;

            if changed {
                score.grade = score.grade(Some(score.accuracy));
                calculations |= Calculations::PP;
            }

            let mut calculator = PPCalculator::new().score(&score).map(map);

            calculator.calculate(calculations).await?;

            let max_pp = calculator.max_pp().unwrap_or(0.0);
            let (stars, pp) = (calculator.stars(), calculator.pp());

            drop(calculator);

            if let Some(stars) = stars {
                score.map.as_mut().unwrap().stars = stars;
            }

            if let Some(pp) = pp {
                score.pp.replace(pp);
            }

            Ok((i + 1, score, Some(max_pp)))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect();

    let mut scores_data: Vec<_> = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sort by adjusted pp
    scores_data.sort_unstable_by(|(_, s1, _), (_, s2, _)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate adjusted pp
    let adjusted_pp: f32 = scores_data
        .iter()
        .map(|(i, Score { pp, .. }, ..)| pp.unwrap_or(0.0) * 0.95_f32.powi(*i as i32 - 1))
        .sum();

    let adjusted_pp = numbers::round((bonus_pp + adjusted_pp).max(0.0) as f32);

    // Accumulate all necessary data
    let content = match args.mods {
        Some(ModSelection::Exact(mods)) => format!(
            "`{name}`{plural} {mode}top100 with only `{mods}` scores:",
            name = user.username,
            plural = plural(user.username.as_str()),
            mode = mode_str(mode),
            mods = mods
        ),
        Some(ModSelection::Exclude(mods)) if mods != NM => {
            let mods: Vec<_> = mods.iter().collect();
            let len = mods.len();
            let mut mod_iter = mods.into_iter();
            let mut mod_str = String::with_capacity(len * 6 - 2);

            if let Some(first) = mod_iter.next() {
                let last = mod_iter.next_back();
                let _ = write!(mod_str, "`{}`", first);

                for elem in mod_iter {
                    let _ = write!(mod_str, ", `{}`", elem);
                }

                if let Some(last) = last {
                    let _ = match len {
                        2 => write!(mod_str, " and `{}`", last),
                        _ => write!(mod_str, ", and `{}`", last),
                    };
                }
            }
            format!(
                "`{name}`{plural} {mode}top100 without {mods}:",
                name = user.username,
                plural = plural(user.username.as_str()),
                mode = mode_str(mode),
                mods = mod_str
            )
        }
        Some(ModSelection::Include(mods)) if mods != NM => format!(
            "`{name}`{plural} {mode}top100 with `{mods}` inserted everywhere:",
            name = user.username,
            plural = plural(user.username.as_str()),
            mode = mode_str(mode),
            mods = mods,
        ),
        _ => format!(
            "`{name}`{plural} top {mode}scores:",
            name = user.username,
            plural = plural(user.username.as_str()),
            mode = mode_str(mode),
        ),
    };

    let pages = numbers::div_euclid(5, scores_data.len());

    let data = TopIfEmbed::new(
        &user,
        scores_data.iter().take(5),
        mode,
        user.statistics.as_ref().unwrap().pp,
        adjusted_pp,
        (1, pages),
    )
    .await;

    // Creating the embed
    let embed = &[data.into_builder().build()];

    let response = msg
        .build_response_msg(&ctx, |m| m.content(&content)?.embeds(embed))
        .await?;

    // * Don't add maps of scores to DB since their stars were potentially changed

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {

        return Ok(());
    }

    // Pagination
    let pre_pp = user.statistics.as_ref().unwrap().pp;
    let pagination = TopIfPagination::new(response, user, scores_data, mode, pre_pp, adjusted_pp);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (topif): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's top plays with(out) the given mods")]
#[long_desc(
    "Display how a user's top plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods]")]
#[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("ti")]
pub async fn topif(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topif_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top taiko plays with(out) the given mods")]
#[long_desc(
    "Display how a user's top taiko plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores\n\
    To exclude converts, specify `-convert` / `-c` as last argument."
)]
#[usage("[username] [mods] [-c]")]
#[example("badewanne3 -hd!", "+hdhr! -c", "whitecat +hddt")]
#[aliases("tit")]
pub async fn topiftaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topif_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's top ctb plays with(out) the given mods")]
#[long_desc(
    "Display how a user's top ctb plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores\n\
    To exclude converts, specify `-convert` / `-c` as last argument."
)]
#[usage("[username] [mods] [-c]")]
#[example("badewanne3 -hd!", "+hdhr! -c", "whitecat +hddt")]
#[aliases("tic")]
pub async fn topifctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    topif_main(GameMode::CTB, ctx, msg, args).await
}

fn plural(name: &str) -> &'static str {
    match name.chars().last() {
        Some('s') => "'",
        Some(_) | None => "'s",
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::TKO => "taiko ",
        GameMode::CTB => "ctb ",
        GameMode::MNA => "mania ",
    }
}

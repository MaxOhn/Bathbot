use crate::{
    arguments::NameModArgs,
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    pp::{Calculations, PPCalculator},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, numbers, osu::ModSelection, MessageExt},
    Args, BotResult, Context,
};

use rosu::model::{GameMode, GameMods, Score};
use std::{cmp::Ordering, collections::HashMap, fmt::Write, sync::Arc};
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
        if mods & ezhr == ezhr {
            content = Some("Looks like an invalid mod combination, EZ and HR exclude each other.");
        }
        let dtht = DT | HT;
        if mods & dtht == dtht {
            content = Some("Looks like an invalid mod combination, DT and HT exclude each other");
        }
        if let Some(content) = content {
            return msg.error(&ctx, content).await;
        }
    }

    // Retrieve the user and their top scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().top_scores(name.as_str()).mode(mode).limit(100);
    let join_result = tokio::try_join!(user_fut, scores_fut);
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    let mut maps = HashMap::new();
    process_tracking(&ctx, mode, &scores, Some(&user), &mut maps).await;

    // Calculate bonus pp
    let actual_pp = scores
        .iter()
        .enumerate()
        .map(|(i, Score { pp, .. })| pp.unwrap() as f64 * 0.95_f64.powi(i as i32))
        .sum::<f64>();
    let bonus_pp = user.pp_raw as f64 - actual_pp;

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().filter_map(|s| s.beatmap_id).collect();
    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            warn!("Error while getting maps from DB: {}", why);
            HashMap::default()
        }
    };

    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());
    let retrieving_msg = if scores.len() - maps.len() > 10 {
        let content = format!(
            "Retrieving {} maps from the api...",
            scores.len() - maps.len()
        );
        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .await
            .ok()
    } else if (mode == GameMode::CTB || mode == GameMode::MNA) && args.mods.is_some() {
        let content = format!("Recalculating top scores, might take a little...");
        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .await
            .ok()
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let mut scores_data = Vec::with_capacity(scores.len());
    let mut missing_maps = Vec::new();
    for (i, score) in scores.into_iter().enumerate() {
        let map_id = score.beatmap_id.unwrap();
        let map = if maps.contains_key(&map_id) {
            maps.remove(&map_id).unwrap()
        } else {
            match ctx.osu().beatmap().map_id(map_id).await {
                Ok(Some(map)) => {
                    missing_maps.push(map.clone());
                    map
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
        };
        scores_data.push((i + 1, score, map));
    }

    // Modify scores
    for (_, score, map) in scores_data.iter_mut() {
        let changed = match args.mods {
            Some(ModSelection::Exact(mods)) => {
                let changed = score.enabled_mods != mods;
                score.enabled_mods = mods;
                changed
            }
            Some(ModSelection::Exclude(mut mods)) if mods != NM => {
                if mods.contains(DT) {
                    mods |= NC;
                }
                if mods.contains(SD) {
                    mods |= PF
                }
                let changed = score.enabled_mods.intersects(mods);
                score.enabled_mods.remove(mods);
                changed
            }
            Some(ModSelection::Include(mods)) if mods != NM => {
                let mut changed = false;
                if mods.contains(DT) && score.enabled_mods.contains(HT) {
                    score.enabled_mods.remove(HT);
                    changed = true;
                }
                if mods.contains(HT) && score.enabled_mods.contains(DT) {
                    score.enabled_mods.remove(NC);
                    changed = true;
                }
                if mods.contains(HR) && score.enabled_mods.contains(EZ) {
                    score.enabled_mods.remove(EZ);
                    changed = true;
                }
                if mods.contains(EZ) && score.enabled_mods.contains(HR) {
                    score.enabled_mods.remove(HR);
                    changed = true;
                }
                changed |= !score.enabled_mods.contains(mods);
                score.enabled_mods.insert(mods);
                changed
            }
            _ => false,
        };
        if changed {
            score.pp = None;
            let mut calculator = PPCalculator::new().score(&*score).map(&*map);
            if let Err(why) = calculator.calculate(Calculations::all(), Some(&ctx)).await {
                warn!("Error while calculating pp for topif {}: {}", mode, why);
            }
            score.pp = calculator
                .pp()
                .map(|val| if val.is_infinite() { 0.0 } else { val });
            score.recalculate_grade(mode, None);
            if score.enabled_mods.changes_stars(mode) {
                if let Some(stars) = calculator.stars() {
                    map.stars = stars;
                }
            }
        }
    }

    // Sort by adjusted pp
    scores_data.sort_unstable_by(|(_, s1, _), (_, s2, _)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate adjusted pp
    let adjusted_pp = scores_data
        .iter()
        .map(|(i, Score { pp, .. }, _)| pp.unwrap_or(0.0) as f64 * 0.95_f64.powi(*i as i32 - 1))
        .sum::<f64>();
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
        &ctx,
        &user,
        scores_data.iter().take(5),
        mode,
        adjusted_pp,
        (1, pages),
    )
    .await;

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Creating the embed
    let embed = data.build().build()?;
    let create_msg = ctx.http.create_message(msg.channel_id).embed(embed)?;
    let response = create_msg.content(content)?.await?;

    // Add missing maps to database
    if !missing_maps.is_empty() {
        match ctx.psql().insert_beatmaps(&missing_maps).await {
            Ok(n) if n < 2 => {}
            Ok(n) => info!("Added {} maps to DB", n),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = TopIfPagination::new(
        Arc::clone(&ctx),
        response,
        user,
        scores_data,
        mode,
        adjusted_pp,
    );
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error (top): {}", why)
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
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods]")]
#[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
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
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods]")]
#[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
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

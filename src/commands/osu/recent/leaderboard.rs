use std::sync::Arc;

use command_macros::command;
use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError, Score};

use crate::{
    commands::{
        osu::{HasMods, ModsResult},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    pagination::LeaderboardPagination,
    pp::PpCalculator,
    util::{
        constants::{AVATAR_URL, OSU_API_ISSUE, OSU_WEB_ISSUE},
        matcher,
        osu::ModSelection,
    },
    BotResult, Context,
};

use super::RecentLeaderboard;

#[command]
#[desc("Global leaderboard of a map that a user recently played")]
#[help(
    "Display the global leaderboard of a map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rlb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rlb", "rglb", "recentgloballeaderboard")]
#[group(Osu)]
async fn prefix_recentleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(None, args);

    leaderboard(ctx, msg.into(), args).await
}

#[command]
#[desc("Global leaderboard of a map that a user recently played")]
#[help(
    "Display the global leaderboard of a mania map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rmlb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rmlb", "rmglb", "recentmaniagloballeaderboard")]
#[group(Mania)]
async fn prefix_recentmanialeaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Mania), args);

    leaderboard(ctx, msg.into(), args).await
}

#[command]
#[desc("Global leaderboard of a map that a user recently played")]
#[help(
    "Display the global leaderboard of a taiko map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rtlb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rtlb", "rtglb", "recenttaikogloballeaderboard")]
#[group(Taiko)]
async fn prefix_recenttaikoleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Taiko), args);

    leaderboard(ctx, msg.into(), args).await
}

#[command]
#[desc("Global leaderboard of a map that a user recently played")]
#[help(
    "Display the global leaderboard of a ctb map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rclb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases(
    "rclb",
    "rcglb",
    "recentctbgloballeaderboard",
    "recentcatchleaderboard"
)]
#[group(Catch)]
async fn prefix_recentctbleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Catch), args);

    leaderboard(ctx, msg.into(), args).await
}

impl<'m> RecentLeaderboard<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;
        let mut mods = None;
        let num = args.num;

        for arg in args.take(2) {
            if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            mode,
            name,
            mods,
            index: num.map(|n| n as usize),
            discord,
        }
    }
}

pub(super) async fn leaderboard(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RecentLeaderboard<'_>,
) -> BotResult<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(&ctx, content).await;
        }
    };

    let limit = args.index.map_or(1, |n| n + (n == 0) as usize);

    if limit > 100 {
        let content = "Recent history goes only 100 scores back.";

        return orig.error(&ctx, content).await;
    }

    let (name, mode) = name_mode!(ctx, orig, args);
    let owner = orig.user_id()?;

    let author_name = if args.name.is_none() && args.discord.is_none() {
        Some(name.clone())
    } else {
        match ctx.user_config(owner).await {
            Ok(config) => config.into_username(),
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to get user config");
                warn!("{report:?}");

                None
            }
        }
    };

    // Retrieve the recent scores
    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .recent()
        .include_fails(true)
        .mode(mode)
        .limit(limit);

    let (map_id, user) = match scores_fut.await {
        Ok(scores) if scores.len() < limit => {
            let content = format!(
                "There are only {} many scores in `{name}`'{} recent history.",
                scores.len(),
                if name.ends_with('s') { "" } else { "s" }
            );

            return orig.error(&ctx, content).await;
        }
        Ok(mut scores) => match scores.pop() {
            Some(score) => {
                let Score { map, user, .. } = score;

                (map.unwrap().map_id, user.unwrap())
            }
            None => {
                let content = format!(
                    "No recent {}plays found for user `{name}`",
                    match mode {
                        GameMode::Osu => "",
                        GameMode::Taiko => "taiko ",
                        GameMode::Catch => "ctb ",
                        GameMode::Mania => "mania ",
                    },
                );

                return orig.error(&ctx, content).await;
            }
        },
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let mods = match mods {
        Some(ModSelection::Exclude(_)) | None => None,
        Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
    };

    // Retrieve the map's leaderboard
    let scores_fut = ctx.client().get_leaderboard(map_id, mods, mode);
    let map_fut = ctx.psql().get_beatmap(map_id, true);

    let (scores_result, map_result) = tokio::join!(scores_fut, map_fut);

    // Retrieving the beatmap
    let mut map = match map_result {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Add map to database if its not in already
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                ctx.map_garbage_collector(&map).execute(&ctx);

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!(
                    "Could not find beatmap with id `{map_id}`. \
                    Did you give me a mapset id instead of a map id?",
                );

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
        },
    };

    if let Some(m) = mods {
        match PpCalculator::new(&ctx, map_id).await {
            Ok(mut calc) => map.stars = calc.mods(m).stars() as f32,
            Err(err) => warn!("{:?}", Report::new(err)),
        }
    }

    let scores = match scores_result {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_WEB_ISSUE).await;

            return Err(err.into());
        }
    };

    let amount = scores.len();

    // Accumulate all necessary data
    let first_place_icon = scores
        .first()
        .map(|_| format!("{AVATAR_URL}{}", user.user_id));

    // Sending the embed
    let content =
        format!("I found {amount} scores with the specified mods on the map's leaderboard");

    LeaderboardPagination::builder(map, scores, author_name, first_place_icon)
        .start_by_update()
        .content(content)
        .start(ctx, orig)
        .await
}

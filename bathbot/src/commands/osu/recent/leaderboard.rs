use std::{collections::HashMap, sync::Arc};

use bathbot_macros::command;
use bathbot_util::{
    constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
    matcher,
    osu::ModSelection,
    IntHasher,
};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, GameModsIntermode, OsuError, Score};

use super::RecentLeaderboard;
use crate::{
    commands::{
        osu::{user_not_found, HasMods, ModsResult},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::osu::UserArgs,
    pagination::LeaderboardPagination,
    Context,
};

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
async fn prefix_recentleaderboard(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
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
) -> Result<()> {
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
) -> Result<()> {
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
) -> Result<()> {
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
) -> Result<()> {
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

    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    // Retrieve the recent scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);

    let scores_fut = ctx
        .osu_scores()
        .recent()
        .limit(1)
        .include_fails(true)
        .exec_with_user(user_args);

    let (map_id, checksum, user) = match scores_fut.await {
        Ok((user, scores)) if scores.len() < limit => {
            let username = user.username();

            let content = format!(
                "There are only {} many scores in `{username}`'{} recent history.",
                scores.len(),
                if username.ends_with('s') { "" } else { "s" }
            );

            return orig.error(&ctx, content).await;
        }
        Ok((user, mut scores)) => match scores.pop() {
            Some(score) => {
                let Score { map, .. } = score;
                let map = map.unwrap();

                (map.map_id, map.checksum, user)
            }
            None => {
                let username = user.username();

                let content = format!(
                    "No recent {}plays found for user `{username}`",
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
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get scores");

            return Err(err);
        }
    };

    let mods = match mods {
        Some(ModSelection::Exclude(_)) | None => None,
        Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
    };

    let scores_fut = ctx
        .client()
        .get_leaderboard::<IntHasher>(map_id, mods.as_ref(), mode);
    let map_fut = ctx.osu_map().map(map_id, checksum.as_deref());

    let (scores_res, map_res) = tokio::join!(scores_fut, map_fut);

    // Retrieving the beatmap
    let map = match map_res {
        Ok(map) => map,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(Report::new(err));
        }
    };

    // Retrieve the map's leaderboard
    let scores = match scores_res {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_WEB_ISSUE).await;

            return Err(err.wrap_err("failed to get scores"));
        }
    };

    let mods_bits = mods.as_ref().map_or(0, GameModsIntermode::bits);

    let mut calc = ctx.pp(&map).mode(map.mode()).mods(mods_bits);
    let attrs = calc.performance().await;

    let amount = scores.len();

    // Accumulate all necessary data
    let first_place_icon = scores
        .first()
        .map(|_| format!("{AVATAR_URL}{}", user.user_id()));

    // Sending the embed
    let content =
        format!("I found {amount} scores with the specified mods on the map's leaderboard");

    let mut attr_map = HashMap::default();
    let stars = attrs.stars() as f32;
    let max_pp = attrs.pp() as f32;
    let max_combo = attrs.max_combo() as u32;
    attr_map.insert(mods_bits, (attrs.into(), max_pp));

    LeaderboardPagination::builder(
        map,
        scores,
        stars,
        max_combo,
        attr_map,
        None, // TODO
        first_place_icon,
    )
    .start_by_update()
    .content(content)
    .start(ctx, orig)
    .await
}

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
    embeds::{EmbedData, LeaderboardEmbed},
    pagination::{LeaderboardPagination, Pagination},
    util::{
        builder::MessageBuilder,
        constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
        matcher, numbers,
        osu::ModSelection,
    },
    BotResult, Context,
};

use super::RecentLeaderboard;

#[command]
#[desc("Belgian leaderboard of a map that a user recently played")]
#[help(
    "Display the belgian leaderboard of a map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[alias("rblb")]
#[group(Osu)]
async fn prefix_recentbelgianleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(None, args);

    leaderboard(ctx, msg.into(), args, true).await
}

#[command]
#[desc("Belgian leaderboard of a map that a user recently played")]
#[help(
    "Display the belgian leaderboard of a mania map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rmblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[alias("rmblb")]
#[group(Mania)]
async fn prefix_recentmaniabelgianleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Mania), args);

    leaderboard(ctx, msg.into(), args, true).await
}

#[command]
#[desc("Belgian leaderboard of a map that a user recently played")]
#[help(
    "Display the belgian leaderboard of a taiko map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rtblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[alias("rtblb")]
#[group(Taiko)]
async fn prefix_recenttaikobelgianleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Taiko), args);

    leaderboard(ctx, msg.into(), args, true).await
}

#[command]
#[desc("Belgian leaderboard of a map that a user recently played")]
#[help(
    "Display the belgian leaderboard of a ctb map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rcblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[alias("rcblb")]
#[group(Catch)]
async fn prefix_recentctbbelgianleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Catch), args);

    leaderboard(ctx, msg.into(), args, true).await
}

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

    leaderboard(ctx, msg.into(), args, false).await
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

    leaderboard(ctx, msg.into(), args, false).await
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

    leaderboard(ctx, msg.into(), args, false).await
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
#[aliases("rclb", "rcglb", "recentctbgloballeaderboard")]
#[group(Catch)]
async fn prefix_recentctbleaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Catch), args);

    leaderboard(ctx, msg.into(), args, false).await
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
    national: bool,
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
        Some(name.clone().into())
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

    let (map, mapset, user) = match scores_fut.await {
        Ok(scores) if scores.len() < limit => {
            let content = format!(
                "There are only {} many scores in `{}`'{} recent history.",
                scores.len(),
                name,
                if name.ends_with('s') { "" } else { "s" }
            );

            return orig.error(&ctx, content).await;
        }
        Ok(mut scores) => match scores.pop() {
            Some(score) => {
                let Score {
                    map, mapset, user, ..
                } = score;

                (map.unwrap(), mapset.unwrap(), user.unwrap())
            }
            None => {
                let content = format!(
                    "No recent {}plays found for user `{}`",
                    match mode {
                        GameMode::STD => "",
                        GameMode::TKO => "taiko ",
                        GameMode::CTB => "ctb ",
                        GameMode::MNA => "mania ",
                    },
                    name
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

    // Retrieve the map's leaderboard
    let scores_fut = ctx.client().get_leaderboard(
        map.map_id,
        national,
        match mods {
            Some(ModSelection::Exclude(_)) | None => None,
            Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
        },
        mode,
    );

    let scores = match scores_fut.await {
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
        .map(|_| format!("{}{}", AVATAR_URL, user.user_id));

    let pages = numbers::div_euclid(10, scores.len());

    let data_fut = LeaderboardEmbed::new(
        author_name.as_deref(),
        &map,
        Some(&mapset),
        (!scores.is_empty()).then(|| scores.iter().take(10)),
        &first_place_icon,
        0,
        &ctx,
        (1, pages),
    );

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    // Sending the embed
    let content =
        format!("I found {amount} scores with the specified mods on the map's leaderboard");

    let embed = embed_data.build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = LeaderboardPagination::new(
        response,
        map,
        Some(mapset),
        scores,
        author_name,
        first_place_icon,
        Arc::clone(&ctx),
    );

    gb.execute(&ctx);

    pagination.start(ctx, owner, 60);

    Ok(())
}

use bathbot_macros::command;
use bathbot_model::command_fields::GameModeOption;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::ModSelection,
};
use eyre::{Report, Result};
use rand::{thread_rng, Rng};
use rosu_v2::{
    prelude::{BeatmapUserScore, GameMode, GameModsIntermode, OsuError, Score, Username},
    request::UserId,
};

use super::RecentLeaderboard;
use crate::{
    active::{impls::LeaderboardPagination, ActiveMessages},
    commands::osu::{
        require_link, user_not_found, HasMods, LeaderboardScore, LeaderboardUserScore, ModsResult,
    },
    core::commands::{prefix::Args, CommandOrigin},
    manager::{
        redis::osu::{UserArgs, UserArgsError},
        Mods,
    },
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
async fn prefix_recentleaderboard(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = RecentLeaderboard::args(None, args);

    leaderboard(msg.into(), args).await
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
async fn prefix_recentmanialeaderboard(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Mania), args);

    leaderboard(msg.into(), args).await
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
async fn prefix_recenttaikoleaderboard(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Taiko), args);

    leaderboard(msg.into(), args).await
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
async fn prefix_recentctbleaderboard(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = RecentLeaderboard::args(Some(GameModeOption::Catch), args);

    leaderboard(msg.into(), args).await
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
            sort: None,
            index: num.to_string_opt().map(String::into),
            discord,
        }
    }
}

pub(super) async fn leaderboard(
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

            return orig.error(content).await;
        }
    };

    let limit = match args.index.as_deref() {
        Some("random" | "?") => thread_rng().gen_range(1..=100),
        Some(n) => match n.parse::<usize>() {
            Ok(n) if n > 100 => {
                let content = "Recent history goes only 100 scores back.";

                return orig.error(content).await;
            }
            Ok(n) => n,
            Err(_) => {
                let content = "Failed to parse index. \
                Must be an integer between 1 and 100 or `random` / `?`.";

                return orig.error(content).await;
            }
        },
        None => 1,
    };

    let owner = orig.user_id()?;

    let config = match Context::user_config().with_osu_id(owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get user config"));
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let user_id = if let Some(user_id) = user_id!(orig, args) {
        user_id
    } else if let Some(user_id) = config.osu {
        UserId::Id(user_id)
    } else {
        return require_link(&orig).await;
    };

    let score_data = match config.score_data {
        Some(score_data) => score_data,
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data)
                .await
                .unwrap_or_default(),
            None => Default::default(),
        },
    };

    let legacy_scores = score_data.is_legacy();

    // Retrieve the recent scores
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let scores_fut = Context::osu_scores()
        .recent(legacy_scores)
        .limit(limit)
        .include_fails(true)
        .exec_with_user(user_args);

    let (map_id, checksum, user) = match scores_fut.await {
        Ok((user, scores)) if scores.len() < limit => {
            let username = user.username.as_str();

            let content = format!(
                "There are only {} many scores in `{username}`'{} recent history.",
                scores.len(),
                if username.ends_with('s') { "" } else { "s" }
            );

            return orig.error(content).await;
        }
        Ok((user, mut scores)) => match scores.pop() {
            Some(score) => {
                let Score { map, .. } = score;
                let map = map.unwrap();

                (map.map_id, map.checksum, user)
            }
            None => {
                let username = user.username.as_str();

                let content = format!(
                    "No recent {}plays found for user `{username}`",
                    match mode {
                        GameMode::Osu => "",
                        GameMode::Taiko => "taiko ",
                        GameMode::Catch => "ctb ",
                        GameMode::Mania => "mania ",
                    },
                );

                return orig.error(content).await;
            }
        },
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get scores");

            return Err(err);
        }
    };

    let specify_mods = match mods {
        Some(ModSelection::Exclude(_)) | None => None,
        Some(ModSelection::Include(ref mods)) | Some(ModSelection::Exact(ref mods)) => {
            Some(mods.to_owned())
        }
    };

    let scores_fut = Context::osu_scores().map_leaderboard(
        map_id,
        mode,
        specify_mods.clone(),
        100,
        legacy_scores,
    );
    let map_fut = Context::osu_map().map(map_id, checksum.as_deref());

    let user_score_fut = get_user_score(
        map_id,
        config.osu,
        mode,
        specify_mods.clone(),
        legacy_scores,
    );

    let (scores_res, map_res, user_score_res) = tokio::join!(scores_fut, map_fut, user_score_fut);

    // Retrieving the beatmap
    let map = match map_res {
        Ok(map) => map,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err));
        }
    };

    let mut scores: Vec<_> = match scores_res {
        Ok(scores) => scores
            .into_iter()
            .enumerate()
            .map(|(i, mut score)| {
                let user = score.user.take();

                LeaderboardScore::new(
                    score.user_id,
                    user.map_or_else(|| "<unknown user>".into(), |user| user.username),
                    score,
                    i + 1,
                )
            })
            .collect(),
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;

            return Err(err.wrap_err("Failed to get scores"));
        }
    };

    let mut user_score = match user_score_res {
        Ok(Some((score, user_id, username))) => Some(LeaderboardUserScore {
            discord_id: owner,
            score: LeaderboardScore::new(user_id, username, score.score, score.pos),
        }),
        Ok(None) => None,
        Err(err) => {
            warn!(?err, "Failed to get user score");

            None
        }
    };

    let mods_ = match specify_mods {
        Some(mods) => Mods {
            clock_rate: Some(mods.legacy_clock_rate()),
            inner: mods.into(),
        },
        None => Mods::default(),
    };

    let mut calc = Context::pp(&map).mode(map.mode()).mods(mods_);
    let attrs = calc.performance().await;

    if let Some(ModSelection::Exclude(ref mods)) = mods {
        if mods.is_empty() {
            scores.retain(|score| !score.mods.is_empty());

            if let Some(ref score) = user_score {
                if score.score.mods.is_empty() {
                    user_score.take();
                }
            }
        } else {
            scores.retain(|score| !score.mods.contains_any(mods.iter()));

            if let Some(ref score) = user_score {
                if score.score.mods.contains_any(mods.iter()) {
                    user_score.take();
                }
            }
        }
    }

    let amount = scores.len();

    let mut content = if mods.is_some() {
        format!("I found {amount} scores with the specified mods on the map's leaderboard")
    } else {
        format!("I found {amount} scores on the map's leaderboard")
    };

    let stars = attrs.stars() as f32;
    let max_combo = attrs.max_combo();

    let order = args.sort.unwrap_or_default();
    order.sort(&mut scores, &map, score_data).await;
    order.push_content(&mut content);

    let first_place_icon = scores.first().map(|_| Box::from(user.avatar_url.as_ref()));

    let pagination = LeaderboardPagination::builder()
        .map(map)
        .scores(scores.into_boxed_slice())
        .stars(stars)
        .max_combo(max_combo)
        .author_data(user_score)
        .first_place_icon(first_place_icon)
        .score_data(score_data)
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

async fn get_user_score(
    map_id: u32,
    user_id: Option<u32>,
    mode: GameMode,
    mods: Option<GameModsIntermode>,
    legacy_scores: bool,
) -> Result<Option<(BeatmapUserScore, u32, Username)>> {
    let Some(user_id) = user_id else {
        return Ok(None);
    };

    let name_fut = Context::osu_user().name(user_id);

    let score_fut =
        Context::osu_scores().user_on_map_single(user_id, map_id, mode, mods, legacy_scores);

    let (score_res, name_res) = tokio::join!(score_fut, name_fut);

    let Some(name) = name_res? else {
        return Ok(None);
    };

    match score_res {
        Ok(score) => Ok(Some((score, user_id, name))),
        Err(OsuError::NotFound) => Ok(None),
        Err(err) => Err(Report::new(err).wrap_err("Failed to get score")),
    }
}

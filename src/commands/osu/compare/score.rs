use std::{borrow::Cow, cmp::Ordering, sync::Arc};

use command_macros::{command, HasMods, HasName, SlashCommand};
use eyre::{Report, Result};
use rosu_v2::prelude::{
    GameMode, OsuError,
    RankStatus::{self, Approved, Loved, Ranked},
    Score,
};
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::message::MessageType,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::osu::{get_user, require_link, HasMods, ModsResult, UserArgs},
    core::commands::{prefix::Args, CommandOrigin},
    database::{EmbedsSize, MinimizedPp},
    embeds::{CompareEmbed, EmbedData, NoScoresEmbed},
    pagination::ScoresPagination,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        matcher,
        osu::{MapIdType, ModSelection},
        InteractionCommandExt, MessageExt,
    },
    Context,
};

use super::{CompareScore, CompareScoreOrder};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "cs",
    help = "Given a user and a map, display the user's scores on the map"
)]
/// Compare a score
pub struct Cs<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<Cow<'a, str>>,
    /// Choose how the scores should be ordered
    sort: Option<CompareScoreOrder>,
    #[command(help = "Filter out scores based on mods.\n\
        Mods must be given as `+mods` to require these mods to be included, \
        `+mods!` to require exactly these mods, \
        or `-mods!` to ignore scores containing any of these mods.\n\
        Examples:\n\
        - `+hd`: Remove scores that don't include `HD`\n\
        - `+hdhr!`: Only keep the `HDHR` score\n\
        - `+nm!`: Only keep the nomod score\n\
        - `-ezhd!`: Remove all scores that have either `EZ` or `HD`")]
    /// Filter out scores based on mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)
    mods: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

#[derive(HasMods, HasName)]
pub(super) struct CompareScoreArgs<'a> {
    name: Option<Cow<'a, str>>,
    map: Option<MapOrScore>,
    sort: Option<CompareScoreOrder>,
    mods: Option<Cow<'a, str>>,
    discord: Option<Id<UserMarker>>,
    index: Option<u64>,
}

impl<'m> CompareScoreArgs<'m> {
    fn args(args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;
        let mut map = None;
        let mut mods = None;
        let index = args.num;

        for arg in args.take(3) {
            if let Some(id) = matcher::get_osu_map_id(arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set))
            {
                map = Some(MapOrScore::Map(id));
            } else if let Some((mode, id)) = matcher::get_osu_score_id(arg) {
                map = Some(MapOrScore::Score { mode, id })
            } else if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            name,
            map,
            sort: None,
            mods,
            discord,
            index,
        }
    }
}

macro_rules! impl_try_from {
    ($($ty:ident),*) => {
        $(
            impl<'a> TryFrom<$ty<'a>> for CompareScoreArgs<'a> {
                type Error = &'static str;

                fn try_from(args: $ty<'a>) -> Result<Self, Self::Error> {
                    let map = if let Some(arg) = args.map {
                        if let Some(id) =
                            matcher::get_osu_map_id(&arg).map(MapIdType::Map).or_else(|| matcher::get_osu_mapset_id(&arg).map(MapIdType::Set))
                        {
                            Some(MapOrScore::Map(id))
                        } else if let Some((mode, id)) = matcher::get_osu_score_id(&arg) {
                            Some(MapOrScore::Score { mode, id })
                        } else {
                            let content =
                                "Failed to parse map url. Be sure you specify a valid map id or url to a map.";

                            return Err(content);
                        }
                    } else {
                        None
                    };

                    Ok(Self {
                        name: args.name,
                        map,
                        sort: args.sort,
                        mods: args.mods,
                        discord: args.discord,
                        index: None,
                    })
                }
            }
        )*
    }
}

impl_try_from!(CompareScore, Cs);

#[command]
#[desc("Compare a player's score on a map")]
#[help(
    "Display a user's top scores on a given map for all mods.\n\
     If mods are specified, only the score with those mods will be shown.\n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel."
)]
#[usage("[username] [map url / map id] [+mods]")]
#[examples(
    "badewanne3",
    "badewanne3 2240404 +eznc",
    "badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
#[aliases("c", "score", "scores")]
#[group(AllModes)]
async fn prefix_compare(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let mut args = CompareScoreArgs::args(args);

    let reply = msg
        .referenced_message
        .as_deref()
        .filter(|_| msg.kind == MessageType::Reply);

    if let Some(msg) = reply {
        if let Some(id) = MapIdType::from_msg(msg) {
            args.map = Some(MapOrScore::Map(id));
        } else if let Some((mode, id)) = matcher::get_osu_score_id(&msg.content) {
            args.map = Some(MapOrScore::Score { id, mode });
        }
    }

    score(ctx, msg.into(), args).await
}

async fn slash_cs(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Cs::from_interaction(command.input_data())?;

    match CompareScoreArgs::try_from(args) {
        Ok(args) => score(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

pub(super) async fn score(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: CompareScoreArgs<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                To specify included mods, provide them e.g. as `+hrdt`.\n\
                For exact mods, provide it e.g. as `+hdhr!`.\n\
                And for excluded mods, provide it e.g. as `-hdnf!`.";

            return orig.error(&ctx, content).await;
        }
    };

    let (name, embeds_size, minimized_pp) = match ctx.user_config(owner).await {
        Ok(config) => match username!(ctx, orig, args) {
            Some(name) => (name, config.score_size, config.minimized_pp),
            None => match config.osu {
                Some(osu) => (osu.into_username(), config.score_size, config.minimized_pp),
                None => return require_link(&ctx, &orig).await,
            },
        },
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get user config"));
        }
    };

    let embeds_size = match (embeds_size, orig.guild_id()) {
        (Some(size), _) => size,
        (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
        (None, None) => EmbedsSize::default(),
    };

    let minimized_pp = match (minimized_pp, orig.guild_id()) {
        (Some(pp), _) => pp,
        (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
        (None, None) => MinimizedPp::default(),
    };

    let CompareScoreArgs {
        sort, map, index, ..
    } = args;

    let map_id = match map {
        Some(MapOrScore::Map(MapIdType::Map(map_id))) => map_id,
        Some(MapOrScore::Map(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(&ctx, content).await;
        }
        Some(MapOrScore::Score { id, mode }) => {
            let mut score = match ctx.osu().score(id, mode).await {
                Ok(score) => score,
                Err(err) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                    let report = Report::new(err).wrap_err("failed to get score");

                    return Err(report);
                }
            };

            let user_fut = ctx.osu().user(score.user_id).mode(mode);

            let pinned_fut = ctx
                .osu()
                .user_scores(score.user_id)
                .pinned()
                .limit(100)
                .mode(mode);

            let (user_result, pinned_result) = tokio::join!(user_fut, pinned_fut);

            match user_result {
                Ok(user) => score.user = Some(user.into()),
                Err(err) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                    let report = Report::new(err).wrap_err("failed to get user");

                    return Err(report);
                }
            }

            let pinned = match pinned_result {
                Ok(scores) => scores.contains(&score),
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get pinned scores");
                    warn!("{report:?}");

                    false
                }
            };

            let map = score.map.as_ref().unwrap();

            let global_idx = if matches!(map.status, Ranked | Loved | Approved) {
                match ctx.osu().beatmap_scores(map.map_id).mode(mode).await {
                    Ok(scores) => scores.iter().position(|s| s == &score),
                    Err(err) => {
                        let report = Report::new(err).wrap_err("Failed to get global scores");
                        warn!("{report:?}");

                        None
                    }
                }
            } else {
                None
            };

            let global_idx = global_idx.map_or(usize::MAX, |idx| idx + 1);
            let mode = score.mode;

            let mut best = if score.map.as_ref().unwrap().status == Ranked {
                let fut = ctx
                    .osu()
                    .user_scores(score.user_id)
                    .best()
                    .limit(100)
                    .mode(mode);

                match fut.await {
                    Ok(scores) => Some(scores),
                    Err(err) => {
                        let report = Report::new(err).wrap_err("Failed to get top scores");
                        warn!("{report:?}");

                        None
                    }
                }
            } else {
                None
            };

            let fut = single_score(
                ctx,
                &orig,
                &score,
                best.as_deref_mut(),
                global_idx,
                pinned,
                embeds_size,
                minimized_pp,
            );

            return fut.await;
        }
        None => {
            let idx = match index {
                Some(_idx @ 51..) => {
                    let content = "I can only go back 50 messages";

                    return orig.error(&ctx, content).await;
                }
                Some(idx) => idx.saturating_sub(1) as usize,
                None => 0,
            };

            let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("failed to retrieve channel history"));
                }
            };

            match MapIdType::map_from_msgs(&msgs, idx) {
                Some(id) => id,
                None if idx == 0 => {
                    let content =
                        "No beatmap specified and none found in recent channel history.\n\
                        Try specifying a map either by url to the map, or just by map id.";

                    return orig.error(&ctx, content).await;
                }
                None => {
                    let content = format!(
                        "No beatmap specified and none found at index {} \
                        of the recent channel history.\nTry decreasing the index or \
                        specify a map either by url to the map, or just by map id.",
                        idx + 1
                    );

                    return orig.error(&ctx, content).await;
                }
            }
        }
    };

    // Retrieving the beatmap
    let mut map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Store map in DB
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", err.wrap_err("Failed to insert map in database"));
                }

                map
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err).wrap_err("failed to get beatmap");

                return Err(report);
            }
        },
    };

    let mut user_args = UserArgs::new(name.as_str(), map.mode);

    let (user, mut scores) = if let Some(alt_name) = user_args.whitespaced_name() {
        match ctx.redis().osu_user(&user_args).await {
            Ok(user) => {
                let scores_fut = ctx
                    .osu()
                    .beatmap_user_scores(map_id, user.user_id)
                    .mode(map.mode);

                match scores_fut.await {
                    Ok(scores) => (user, scores),
                    Err(OsuError::NotFound) => {
                        let content = "Beatmap was not found. Maybe unranked?";

                        return orig.error(&ctx, content).await;
                    }
                    Err(err) => {
                        let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                        let report = Report::new(err).wrap_err("failed to get user score");

                        return Err(report);
                    }
                }
            }
            Err(OsuError::NotFound) => {
                user_args.name = &alt_name;
                let redis = ctx.redis();

                let scores_fut = ctx
                    .osu()
                    .beatmap_user_scores(map_id, alt_name.as_str())
                    .mode(map.mode);

                match tokio::join!(redis.osu_user(&user_args), scores_fut) {
                    (Err(OsuError::NotFound), _) => {
                        let content = format!("User `{name}` was not found");

                        return orig.error(&ctx, content).await;
                    }
                    (_, Err(OsuError::NotFound)) => {
                        let content = "Beatmap was not found. Maybe unranked?";

                        return orig.error(&ctx, content).await;
                    }
                    (Err(err), _) | (_, Err(err)) => {
                        let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                        let report = Report::new(err).wrap_err("failed to get user or scores");

                        return Err(report);
                    }
                    (Ok(user), Ok(scores)) => (user, scores),
                }
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err).wrap_err("failed to get user");

                return Err(report);
            }
        }
    } else {
        let scores_fut = ctx
            .osu()
            .beatmap_user_scores(map_id, name.as_str())
            .mode(map.mode);

        let redis = ctx.redis();

        match tokio::join!(redis.osu_user(&user_args), scores_fut) {
            (Err(OsuError::NotFound), _) => {
                let content = format!("User `{name}` was not found");

                return orig.error(&ctx, content).await;
            }
            (_, Err(OsuError::NotFound)) => {
                let content = "Beatmap was not found. Maybe unranked?";

                return orig.error(&ctx, content).await;
            }
            (Err(err), _) | (_, Err(err)) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err).wrap_err("failed to get user or scores");

                return Err(report);
            }
            (Ok(user), Ok(scores)) => (user, scores),
        }
    };

    match mods {
        Some(ModSelection::Include(mods)) if mods.is_empty() => {
            scores.retain(|s| s.mods.is_empty())
        }
        Some(ModSelection::Include(mods)) => scores.retain(|s| s.mods.contains(mods)),
        Some(ModSelection::Exact(mods)) => scores.retain(|s| s.mods == mods),
        Some(ModSelection::Exclude(mods)) => {
            scores.retain(|s| s.mods.intersection(mods).is_empty())
        }
        None => {}
    }

    if scores.is_empty() {
        return no_scores(&ctx, &orig, name.as_str(), map_id, mods).await;
    }

    let pinned_fut = ctx
        .osu()
        .user_scores(user.user_id)
        .pinned()
        .mode(map.mode)
        .limit(100);

    let sort_fut = sort
        .unwrap_or_default()
        .apply(&ctx, &mut scores, map.map_id);

    let global_fut = async {
        if matches!(
            map.status,
            RankStatus::Ranked | RankStatus::Loved | RankStatus::Approved
        ) {
            let fut = ctx.osu().beatmap_scores(map.map_id).mode(map.mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let personal_fut = async {
        if map.status == RankStatus::Ranked {
            let fut = ctx
                .osu()
                .user_scores(user.user_id)
                .mode(map.mode)
                .best()
                .limit(100);

            Some(fut.await)
        } else {
            None
        }
    };

    let (pinned_result, _, global_result, personal_result) =
        tokio::join!(pinned_fut, sort_fut, global_fut, personal_fut);

    let pinned = match pinned_result {
        Ok(scores) => scores,
        Err(err) => {
            let report = Report::new(err).wrap_err("Failed to get pinned scores");
            warn!("{report:?}");

            Vec::new()
        }
    };

    // First elem: idx inside user scores that has most score
    // Second elem: idx of score inside map leaderboard
    let global_idx = match global_result {
        Some(Ok(globals)) => scores
            .iter()
            .enumerate()
            .max_by_key(|(_, s)| s.score)
            .and_then(|(i, s)| {
                let user = user.user_id;
                let timestamp = s.ended_at.unix_timestamp();

                globals
                    .iter()
                    .position(|s| s.ended_at.unix_timestamp() == timestamp && s.user_id == user)
                    .map(|pos| (i, pos + 1))
            }),
        Some(Err(err)) => {
            let report = Report::new(err).wrap_err("Failed to get map leaderboard");
            warn!("{report:?}");

            None
        }
        None => None,
    };

    let mut personal = match personal_result {
        Some(Ok(scores)) => scores,
        Some(Err(err)) => {
            let report = Report::new(err).wrap_err("Failed to get top scores");
            warn!("{report:?}");

            Vec::new()
        }
        None => Vec::new(),
    };

    if let [score] = &mut scores[..] {
        let global_idx = global_idx.map_or(usize::MAX, |(_, i)| i);
        let best = (!personal.is_empty()).then(|| &mut personal[..]);
        let pinned = pinned.contains(score);
        score.user = Some(user.into());
        score.mapset = map.mapset.take().map(From::from);
        score.map = Some(map);

        let fut = single_score(
            ctx,
            &orig,
            score,
            best,
            global_idx,
            pinned,
            embeds_size,
            minimized_pp,
        );

        fut.await
    } else {
        let pp_idx = scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.pp.partial_cmp(&b.pp).unwrap_or(Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        let builder =
            ScoresPagination::builder(user, map, scores, pinned, personal, global_idx, pp_idx);

        builder
            .start_by_update()
            .defer_components()
            .start(ctx, orig)
            .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn single_score(
    ctx: Arc<Context>,
    orig: &CommandOrigin<'_>,
    score: &Score,
    best: Option<&mut [Score]>,
    global_idx: usize,
    pinned: bool,
    embeds_size: EmbedsSize,
    minimized_pp: MinimizedPp,
) -> Result<()> {
    // Accumulate all necessary data
    let embed_fut = CompareEmbed::new(
        best.as_deref(),
        score,
        global_idx,
        pinned,
        minimized_pp,
        &ctx,
    );

    let embed_data = match embed_fut.await {
        Ok(data) => data,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to create embed"));
        }
    };

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let builder = embed_data.into_minimized().into();
            orig.create_message(&ctx, &builder).await?;
        }
        EmbedsSize::InitialMaximized => {
            let builder = embed_data.as_maximized().into();
            let response = orig.create_message(&ctx, &builder).await?.model().await?;

            ctx.store_msg(response.id);
            let ctx = Arc::clone(&ctx);

            // Wait for minimizing
            tokio::spawn(async move {
                sleep(Duration::from_secs(45)).await;

                if !ctx.remove_msg(response.id) {
                    return;
                }

                let builder = embed_data.into_minimized().into();

                if let Err(err) = response.update(&ctx, &builder).await {
                    let report = Report::new(err).wrap_err("Failed to minimize message");
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let builder = embed_data.as_maximized().into();
            orig.create_message(&ctx, &builder).await?;
        }
    }

    // Process user and their top scores for tracking
    #[cfg(feature = "osutracking")]
    if let Some(scores) = best {
        crate::tracking::process_osu_tracking(&ctx, scores, None).await;
    }

    Ok(())
}

async fn no_scores(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
    map_id: u32,
    mods: Option<ModSelection>,
) -> Result<()> {
    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", err.wrap_err("Failed to insert map in database"));
                }

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!("There is no map with id {map_id}");

                return orig.error(ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(ctx, OSU_API_ISSUE).await;
                let report = Report::new(err).wrap_err("failed to get beatmap");

                return Err(report);
            }
        },
    };

    let user_args = UserArgs::new(name, map.mode);

    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");

            return orig.error(ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    // Sending the embed
    let embed = NoScoresEmbed::new(user, map, mods).build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(ctx, &builder).await?;

    Ok(())
}

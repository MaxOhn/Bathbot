use std::{cmp::Ordering, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{
    GameMode, OsuError,
    RankStatus::{self, Approved, Loved, Ranked},
    Score,
};
use tokio::time::{sleep, Duration};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    channel::message::MessageType,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user, UserArgs},
        parse_discord, DoubleResultCow, MyCommand,
    },
    database::{EmbedsSize, MinimizedPp, UserConfig},
    embeds::{CompareEmbed, EmbedData, NoScoresEmbed, ScoresEmbed},
    error::Error,
    pagination::{Pagination, ScoresPagination},
    tracking::process_osu_tracking,
    util::{
        constants::{
            common_literals::{ACC, COMBO, DISCORD, MAP, MAP_PARSE_FAIL, MODS, NAME, SORT},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher, numbers,
        osu::{map_id_from_history, map_id_from_msg, MapIdType, ModSelection},
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use super::{score_options, ScoreOrder};

#[command]
#[short_desc("Compare a player's score on a map")]
#[long_desc(
    "Display a user's top score on a given map. \n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel."
)]
#[usage("[username] [map url / map id]")]
#[example(
    "badewanne3",
    "badewanne3 2240404",
    "badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
#[aliases("c", "score", "scores")]
async fn compare(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ScoreArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut score_args)) => {
                    let reply = msg
                        .referenced_message
                        .as_ref()
                        .filter(|_| msg.kind == MessageType::Reply);

                    if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                        score_args.id = Some(MapOrScore::Map(id));
                    } else if let Some((mode, id)) =
                        reply.and_then(|msg| matcher::get_osu_score_id(&msg.content))
                    {
                        score_args.id = Some(MapOrScore::Score { id, mode });
                    }

                    _compare(ctx, CommandData::Message { msg, args, num }, score_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_compare(ctx, *command).await,
    }
}

pub(super) async fn _compare(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: ScoreArgs,
) -> BotResult<()> {
    let ScoreArgs {
        config,
        id,
        mods,
        sort_by,
    } = args;

    let embeds_size = match (config.embeds_size, data.guild_id()) {
        (Some(size), _) => size,
        (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
        (None, None) => EmbedsSize::default(),
    };

    let minimized_pp = match (config.minimized_pp, data.guild_id()) {
        (Some(pp), _) => pp,
        (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
        (None, None) => MinimizedPp::default(),
    };

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let map_id = match id {
        Some(MapOrScore::Map(MapIdType::Map(map_id))) => map_id,
        Some(MapOrScore::Map(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return data.error(&ctx, content).await;
        }
        Some(MapOrScore::Score { id, mode }) => {
            let mut score = match ctx.osu().score(id, mode).await {
                Ok(score) => score,
                Err(err) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
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
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
                }
            }

            let pinned = match pinned_result {
                Ok(scores) => scores.contains(&score),
                Err(err) => {
                    let report = Report::new(err).wrap_err("failed to retrieve pinned scores");
                    warn!("{report:?}");

                    false
                }
            };

            let map = score.map.as_ref().unwrap();

            let global_idx = if matches!(map.status, Ranked | Loved | Approved) {
                match ctx.osu().beatmap_scores(map.map_id).mode(mode).await {
                    Ok(scores) => scores.iter().position(|s| s == &score),
                    Err(err) => {
                        let report = Report::new(err).wrap_err("failed to get global scores");
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
                        let report = Report::new(err).wrap_err("failed to get top scores");
                        warn!("{report:?}");

                        None
                    }
                }
            } else {
                None
            };

            let fut = single_score(
                ctx,
                &data,
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
            let msgs = match ctx.retrieve_channel_history(data.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            };

            let map_id = match map_id_from_history(&msgs) {
                Some(MapIdType::Map(id)) => id,
                Some(MapIdType::Set(_)) => {
                    let content = "I found a mapset in the channel history but I need a map. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return data.error(&ctx, content).await;
                }
                None => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return data.error(&ctx, content).await;
                }
            };

            map_id
        }
    };

    // Retrieving the beatmap
    let mut map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Store map in DB
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(err) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
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
                    Err(err) => {
                        let _ = data.error(&ctx, OSU_API_ISSUE).await;

                        return Err(err.into());
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

                        return data.error(&ctx, content).await;
                    }
                    (Err(err), _) => {
                        let _ = data.error(&ctx, OSU_API_ISSUE).await;

                        return Err(err.into());
                    }
                    (_, Err(err)) => {
                        let _ = data.error(&ctx, OSU_API_ISSUE).await;

                        return Err(err.into());
                    }
                    (Ok(user), Ok(scores)) => (user, scores),
                }
            }
            Err(err) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
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

                return data.error(&ctx, content).await;
            }
            (Err(err), _) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
            (_, Err(err)) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
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
        return no_scores(&ctx, &data, name.as_str(), map_id, mods).await;
    }

    let pinned_fut = ctx
        .osu()
        .user_scores(user.user_id)
        .pinned()
        .mode(map.mode)
        .limit(100);

    let sort_fut = sort_by.apply(&ctx, &mut scores, map.map_id);

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
            let report = Report::new(err).wrap_err("failed to get pinned scores");
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
                let timestamp = s.created_at.timestamp();

                globals
                    .iter()
                    .position(|s| s.created_at.timestamp() == timestamp && s.user_id == user)
                    .map(|pos| (i, pos + 1))
            }),
        Some(Err(err)) => {
            let report = Report::new(err).wrap_err("failed to get map leaderboard");
            warn!("{report:?}");

            None
        }
        None => None,
    };

    let mut personal = match personal_result {
        Some(Ok(scores)) => scores,
        Some(Err(err)) => {
            let report = Report::new(err).wrap_err("failed to get top100");
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
            &data,
            score,
            best,
            global_idx,
            pinned,
            embeds_size,
            minimized_pp,
        );

        return fut.await;
    } else {
        let pages = numbers::div_euclid(10, scores.len());
        let init_scores = scores.iter().take(10);

        let pp_idx = scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.pp.partial_cmp(&b.pp).unwrap_or(Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Accumulate all necessary data
        let embed_fut = ScoresEmbed::new(
            &user,
            &map,
            init_scores,
            &pinned,
            &personal,
            global_idx,
            pp_idx,
            (1, pages),
            &ctx,
        );

        let builder = embed_fut.await.into_builder().build().into();
        let response_raw = data.create_message(&ctx, builder).await?;

        // Skip pagination if too few entries
        if scores.len() <= 10 {
            return Ok(());
        }

        let response = response_raw.model().await?;

        // Pagination
        let pagination = ScoresPagination::new(
            response,
            user,
            map,
            scores,
            pinned,
            personal,
            global_idx,
            pp_idx,
            Arc::clone(&ctx),
        );
        let owner = data.author()?.id;

        tokio::spawn(async move {
            if let Err(err) = pagination.start(&ctx, owner, 60).await {
                warn!("{:?}", Report::new(err));
            }
        });
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn single_score(
    ctx: Arc<Context>,
    data: &CommandData<'_>,
    score: &Score,
    best: Option<&mut [Score]>,
    global_idx: usize,
    pinned: bool,
    embeds_size: EmbedsSize,
    minimized_pp: MinimizedPp,
) -> BotResult<()> {
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
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let builder = embed_data.into_builder().build().into();
            data.create_message(&ctx, builder).await?;
        }
        EmbedsSize::InitialMaximized => {
            let builder = embed_data.as_builder().build().into();
            let response = data.create_message(&ctx, builder).await?.model().await?;

            ctx.store_msg(response.id);
            let ctx = Arc::clone(&ctx);

            // Wait for minimizing
            tokio::spawn(async move {
                sleep(Duration::from_secs(45)).await;

                if !ctx.remove_msg(response.id) {
                    return;
                }

                let builder = embed_data.into_builder().build().into();

                if let Err(err) = response.update_message(&ctx, builder).await {
                    let report = Report::new(err).wrap_err("failed to minimize message");
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let builder = embed_data.as_builder().build().into();
            data.create_message(&ctx, builder).await?;
        }
    }

    // Process user and their top scores for tracking
    if let Some(scores) = best {
        process_osu_tracking(&ctx, scores, None).await;
    }

    Ok(())
}

async fn no_scores(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    map_id: u32,
    mods: Option<ModSelection>,
) -> BotResult<()> {
    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!("There is no map with id {map_id}");

                return data.error(ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    let user_args = UserArgs::new(name, map.mode);

    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");

            return data.error(ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Sending the embed
    let embed = NoScoresEmbed::new(user, map, mods).into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(ctx, builder).await?;

    Ok(())
}

enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

pub async fn slash_cs(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let options = command.yoink_options();

    match ScoreArgs::slash(&ctx, &command, options).await? {
        Ok(args) => _compare(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub(super) struct ScoreArgs {
    config: UserConfig,
    id: Option<MapOrScore>,
    mods: Option<ModSelection>,
    sort_by: ScoreOrder,
}

impl ScoreArgs {
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        To specify included mods, provide them e.g. as `+hrdt`.\n\
        For exact mods, provide it e.g. as `+hdhr!`.\n\
        And for excluded mods, provide it e.g. as `-hdnf!`.";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut id = None;
        let mut mods = None;

        for arg in args.take(3) {
            if let Some(id_) =
                matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
            {
                id = Some(MapOrScore::Map(id_));
            } else if let Some((mode, id_)) = matcher::get_osu_score_id(arg) {
                id = Some(MapOrScore::Score { mode, id: id_ })
            } else if let Some(mods_) = matcher::get_mods(arg) {
                mods = Some(mods_);
            } else {
                match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        let sort_by = ScoreOrder::Score;

        Ok(Ok(Self {
            config,
            id,
            mods,
            sort_by,
        }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut id = None;
        let mut mods = None;
        let mut sort_by = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MAP => match matcher::get_osu_map_id(&value)
                        .or_else(|| matcher::get_osu_mapset_id(&value))
                    {
                        Some(id_) => id = Some(MapOrScore::Map(id_)),
                        None => match matcher::get_osu_score_id(&value) {
                            Some((mode, id_)) => id = Some(MapOrScore::Score { mode, id: id_ }),
                            None => return Ok(Err(MAP_PARSE_FAIL.into())),
                        },
                    },
                    SORT => match value.as_str() {
                        ACC => sort_by = Some(ScoreOrder::Acc),
                        COMBO => sort_by = Some(ScoreOrder::Combo),
                        "date" => sort_by = Some(ScoreOrder::Date),
                        "miss" => sort_by = Some(ScoreOrder::Misses),
                        "pp" => sort_by = Some(ScoreOrder::Pp),
                        "score" => sort_by = Some(ScoreOrder::Score),
                        "stars" => sort_by = Some(ScoreOrder::Stars),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
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

        Ok(Ok(ScoreArgs {
            config,
            id,
            mods,
            sort_by: sort_by.unwrap_or_default(),
        }))
    }
}

pub fn define_cs() -> MyCommand {
    let score_help = "Given a user and a map, display the user's scores on the map";

    MyCommand::new("cs", "Compare a score")
        .help(score_help)
        .options(score_options())
}

use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use bathbot_macros::{command, HasMods, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_psql::model::{
    configs::{GuildConfig, MinimizedPp, ScoreSize},
    osu::{ArchivedMapVersion, MapVersion},
};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::{MapIdType, ModSelection},
    CowUtils, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{
        GameMode, GameMods, Grade, OsuError,
        RankStatus::{self, Approved, Loved, Ranked},
        Score,
    },
    request::UserId,
};
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{AutocompleteValue, CommandModel, CreateCommand};
use twilight_model::{
    application::command::CommandOptionChoice,
    channel::message::MessageType,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::osu::{require_link, HasMods, ModsResult},
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{CompareEmbed, EmbedData, NoScoresEmbed},
    manager::{
        redis::{
            osu::{User, UserArgs, UserArgsSlim},
            RedisData,
        },
        MapError, OsuMap,
    },
    pagination::ScoresPagination,
    util::{interaction::InteractionCommand, osu::IfFc, InteractionCommandExt, MessageExt},
    Context,
};

use super::{CompareScoreAutocomplete, ScoreOrder};

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "cs",
    help = "Given a user and a map, display the user's scores on the map"
)]
#[allow(dead_code)]
/// Compare a score
pub struct Cs<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<Cow<'a, str>>,
    #[command(autocomplete = true)]
    /// Specify a difficulty name of the map's mapset
    difficulty: Option<String>,
    /// Choose how the scores should be ordered
    sort: Option<ScoreOrder>,
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

#[derive(CommandModel, HasName)]
#[command(autocomplete = true)]
pub struct CsAutocomplete<'a> {
    name: Option<Cow<'a, str>>,
    map: Option<Cow<'a, str>>,
    difficulty: AutocompleteValue<String>,
    sort: Option<ScoreOrder>,
    mods: Option<Cow<'a, str>>,
    discord: Option<Id<UserMarker>>,
}

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "score",
    help = "Given a user and a map, display the user's scores on the map.\n\
        Its shorter alias is the `/cs` command."
)]
#[allow(dead_code)]
/// Compare a score
pub struct CompareScore_<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<Cow<'a, str>>,
    #[command(autocomplete = true)]
    /// Specify a difficulty name of the map's mapset
    difficulty: Option<String>,
    /// Choose how the scores should be ordered
    sort: Option<ScoreOrder>,
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

#[derive(CommandModel, HasName)]
#[command(autocomplete = true)]
pub struct CompareScoreAutocomplete_<'a> {
    name: Option<Cow<'a, str>>,
    map: Option<Cow<'a, str>>,
    difficulty: AutocompleteValue<String>,
    sort: Option<ScoreOrder>,
    mods: Option<Cow<'a, str>>,
    discord: Option<Id<UserMarker>>,
}

pub enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

#[derive(HasMods, HasName)]
pub(super) struct CompareScoreArgs<'a> {
    name: Option<Cow<'a, str>>,
    map: Option<MapOrScore>,
    difficulty: Option<String>,
    sort: Option<ScoreOrder>,
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
            difficulty: None,
            sort: None,
            mods,
            discord,
            index,
        }
    }
}

macro_rules! impl_try_from {
    ( $( $ty:ident ,)* ) => {
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

                    let difficulty = match args.difficulty {
                        AutocompleteValue::None |
                        AutocompleteValue::Focused(_) => None,
                        AutocompleteValue::Completed(diff) => Some(diff),
                    };

                    Ok(Self {
                        name: args.name,
                        map,
                        difficulty,
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

impl_try_from!(
    CompareScoreAutocomplete,
    CsAutocomplete,
    CompareScoreAutocomplete_,
);

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

pub async fn slash_cs(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = CsAutocomplete::from_interaction(command.input_data())?;

    match args.difficulty {
        AutocompleteValue::None => {
            return handle_autocomplete(&ctx, &command, None, &args.map).await
        }
        AutocompleteValue::Focused(diff) => {
            return handle_autocomplete(&ctx, &command, Some(diff), &args.map).await
        }
        AutocompleteValue::Completed(_) => {}
    }

    match CompareScoreArgs::try_from(args) {
        Ok(args) => score(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_comparescore_(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = CompareScoreAutocomplete_::from_interaction(command.input_data())?;

    match args.difficulty {
        AutocompleteValue::None => {
            return handle_autocomplete(&ctx, &command, None, &args.map).await
        }
        AutocompleteValue::Focused(diff) => {
            return handle_autocomplete(&ctx, &command, Some(diff), &args.map).await
        }
        AutocompleteValue::Completed(_) => {}
    }

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

    let (user_id, score_size, minimized_pp) = match ctx.user_config().with_osu_id(owner).await {
        Ok(config) => match user_id!(ctx, orig, args) {
            Some(user_id) => (user_id, config.score_size, config.minimized_pp),
            None => match config.osu {
                Some(user_id) => (UserId::Id(user_id), config.score_size, config.minimized_pp),
                None => return require_link(&ctx, &orig).await,
            },
        },
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get user config"));
        }
    };

    let (guild_score_size, guild_minimized_pp) = match orig.guild_id() {
        Some(guild_id) => {
            let f = |config: &GuildConfig| (config.score_size, config.minimized_pp);

            ctx.guild_config().peek(guild_id, f).await
        }
        None => (None, None),
    };

    let score_size = score_size.or(guild_score_size).unwrap_or_default();
    let minimized_pp = minimized_pp.or(guild_minimized_pp).unwrap_or_default();

    let CompareScoreArgs {
        sort,
        map,
        index,
        difficulty,
        ..
    } = args;

    let map_id = if let Some(Ok(map_id)) = difficulty.as_deref().map(str::parse) {
        map_id
    } else {
        match map {
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

                let map = score.map.take().expect("missing map");
                let map_fut = ctx.osu_map().map(map.map_id, map.checksum.as_deref());

                let user_args = UserArgs::user_id(score.user_id).mode(mode);
                let user_fut = ctx.redis().osu_user(user_args);

                let pinned_fut = ctx
                    .osu()
                    .user_scores(score.user_id)
                    .pinned()
                    .limit(100)
                    .mode(mode);

                let (user_res, map_res, pinned_res) = tokio::join!(user_fut, map_fut, pinned_fut);

                let user = match user_res {
                    Ok(user) => user,
                    Err(err) => {
                        let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                        let err = Report::new(err).wrap_err("failed to get user");

                        return Err(err);
                    }
                };

                let map = match map_res {
                    Ok(map) => map,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(Report::new(err));
                    }
                };

                let pinned = match pinned_res {
                    Ok(scores) => scores.contains(&score),
                    Err(err) => {
                        let err = Report::new(err).wrap_err("failed to get pinned scores");
                        warn!("{err:?}");

                        false
                    }
                };

                let status = map.status();

                let global_idx = if matches!(status, Ranked | Loved | Approved) {
                    match ctx.osu().beatmap_scores(map.map_id()).mode(mode).await {
                        Ok(scores) => scores.iter().position(|s| s == &score),
                        Err(err) => {
                            let err = Report::new(err).wrap_err("failed to get global scores");
                            warn!("{err:?}");

                            None
                        }
                    }
                } else {
                    None
                };

                let global_idx = global_idx.map_or(usize::MAX, |idx| idx + 1);
                let mode = score.mode;

                let best = if status == Ranked {
                    let fut = ctx
                        .osu()
                        .user_scores(score.user_id)
                        .best()
                        .limit(100)
                        .mode(mode);

                    match fut.await {
                        Ok(scores) => Some(scores),
                        Err(err) => {
                            let err = Report::new(err).wrap_err("failed to get top scores");
                            warn!("{err:?}");

                            None
                        }
                    }
                } else {
                    None
                };

                let mut calc = ctx.pp(&map).mode(score.mode).mods(score.mods);
                let attrs = calc.performance().await;

                let max_pp = score
                    .pp
                    .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
                    .unwrap_or(attrs.pp() as f32);

                let pp = match score.pp {
                    Some(pp) => pp,
                    None => calc.score(&score).performance().await.pp() as f32,
                };

                let score = ScoreSlim::new(score, pp);
                let if_fc = IfFc::new(&ctx, &score, &map).await;

                let entry = CompareEntry {
                    score,
                    stars: attrs.stars() as f32,
                    max_pp,
                    if_fc,
                };

                let fut = single_score(
                    ctx,
                    &orig,
                    &entry,
                    &user,
                    &map,
                    best.as_deref(),
                    global_idx,
                    pinned,
                    score_size,
                    minimized_pp,
                );

                return fut.await;
            }
            None => {
                let idx = match index {
                    Some(51..) => {
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
        }
    };

    // Retrieving the beatmap
    let map = match ctx.osu_map().map(map_id, None).await {
        Ok(map) => map,
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?"
            );

            return orig.error(&ctx, content).await;
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = map.mode();
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);

    let (user_res, score_res) = match user_args {
        UserArgs::Args(args) => {
            let user_fut = ctx.redis().osu_user_from_args(args);
            let score_fut = ctx.osu_scores().user_on_map(map_id).exec(args);

            tokio::join!(user_fut, score_fut)
        }
        UserArgs::User { user, mode } => {
            let args = UserArgsSlim::user_id(user.user_id).mode(mode);
            let user = RedisData::Original(*user);
            let score_res = ctx.osu_scores().user_on_map(map_id).exec(args).await;

            (Ok(user), score_res)
        }
        UserArgs::Err(err) => (Err(err), Ok(Vec::new())),
    };

    let (user, scores) = match (user_res, score_res) {
        (Ok(user), Ok(scores)) => (user, scores),
        (Err(OsuError::NotFound), _) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(&ctx, content).await;
        }
        (_, Err(OsuError::NotFound)) => {
            let content = "Beatmap was not found. Maybe unranked?";

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    let entries = match process_scores(&ctx, map_id, scores, mods, sort.unwrap_or_default()).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to process scores"));
        }
    };

    if entries.is_empty() {
        let embed = NoScoresEmbed::new(&user, &map, mods).build();
        let builder = MessageBuilder::new().embed(embed);
        orig.create_message(&ctx, &builder).await?;

        return Ok(());
    }

    let pinned_fut = ctx
        .osu()
        .user_scores(user.user_id())
        .pinned()
        .mode(mode)
        .limit(100);

    let global_fut = async {
        if matches!(
            map.status(),
            RankStatus::Ranked | RankStatus::Loved | RankStatus::Approved
        ) {
            let fut = ctx.osu().beatmap_scores(map_id).mode(mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let personal_fut = async {
        if map.status() == RankStatus::Ranked {
            let fut = ctx
                .osu()
                .user_scores(user.user_id())
                .mode(mode)
                .best()
                .limit(100);

            Some(fut.await)
        } else {
            None
        }
    };

    let (pinned_res, global_res, personal_res) = tokio::join!(pinned_fut, global_fut, personal_fut);

    let pinned = match pinned_res {
        Ok(scores) => scores,
        Err(err) => {
            let err = Report::new(err).wrap_err("failed to get pinned scores");
            warn!("{err:?}");

            Vec::new()
        }
    };

    // First elem: idx inside user scores that has most score
    // Second elem: idx of score inside map leaderboard
    let global_idx = match global_res {
        Some(Ok(globals)) => entries
            .iter()
            .enumerate()
            .max_by_key(|(_, entry)| entry.score.score)
            .and_then(|(i, entry)| {
                let user = user.user_id();
                let timestamp = entry.score.ended_at.unix_timestamp();

                globals
                    .iter()
                    .position(|s| s.ended_at.unix_timestamp() == timestamp && s.user_id == user)
                    .map(|pos| (i, pos + 1))
            }),
        Some(Err(err)) => {
            let err = Report::new(err).wrap_err("failed to get map leaderboard");
            warn!("{err:?}");

            None
        }
        None => None,
    };

    let personal = match personal_res {
        Some(Ok(scores)) => scores,
        Some(Err(err)) => {
            let err = Report::new(err).wrap_err("failed to get top scores");
            warn!("{err:?}");

            Vec::new()
        }
        None => Vec::new(),
    };

    if let [entry] = &entries[..] {
        let global_idx = global_idx.map_or(usize::MAX, |(_, i)| i);
        let best = (!personal.is_empty()).then(|| &personal[..]);

        let pinned = pinned.iter().any(|pinned| {
            (pinned.ended_at.unix_timestamp() - entry.score.ended_at.unix_timestamp()).abs() <= 2
        });

        let fut = single_score(
            ctx,
            &orig,
            entry,
            &user,
            &map,
            best,
            global_idx,
            pinned,
            score_size,
            minimized_pp,
        );

        fut.await
    } else {
        let pp_idx = entries
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.score
                    .pp
                    .partial_cmp(&b.score.pp)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        let builder =
            ScoresPagination::builder(user, map, entries, pinned, personal, global_idx, pp_idx);

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
    entry: &CompareEntry,
    user: &RedisData<User>,
    map: &OsuMap,
    best: Option<&[Score]>,
    global_idx: usize,
    pinned: bool,
    embeds_size: ScoreSize,
    minimized_pp: MinimizedPp,
) -> Result<()> {
    // Accumulate all necessary data
    let embed_data = CompareEmbed::new(best, entry, user, map, global_idx, pinned, minimized_pp);

    // Only maximize if config allows it
    match embeds_size {
        ScoreSize::AlwaysMinimized => {
            let builder = embed_data.into_minimized().into();
            orig.create_message(&ctx, &builder).await?;
        }
        ScoreSize::InitialMaximized => {
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
                    let err = Report::new(err).wrap_err("failed to minimize message");
                    warn!("{err:?}");
                }
            });
        }
        ScoreSize::AlwaysMaximized => {
            let builder = embed_data.as_maximized().into();
            orig.create_message(&ctx, &builder).await?;
        }
    }

    Ok(())
}

pub struct CompareEntry {
    pub score: ScoreSlim,
    pub stars: f32,
    pub max_pp: f32,
    pub if_fc: Option<IfFc>,
}

async fn process_scores(
    ctx: &Context,
    map_id: u32,
    mut scores: Vec<Score>,
    mods: Option<ModSelection>,
    sort: ScoreOrder,
) -> Result<Vec<CompareEntry>> {
    let mut entries = Vec::with_capacity(scores.len());
    let map = ctx.osu_map().map(map_id, None).await?;

    match mods {
        None => {}
        Some(ModSelection::Include(mods @ GameMods::NoMod) | ModSelection::Exact(mods)) => {
            scores.retain(|score| score.mods == mods);
        }
        Some(ModSelection::Include(mods)) => scores.retain(|score| score.mods.contains(mods)),
        Some(ModSelection::Exclude(GameMods::NoMod)) => {
            scores.retain(|score| !score.mods.is_empty())
        }
        Some(ModSelection::Exclude(mods)) => scores.retain(|score| !score.mods.intersects(mods)),
    }

    for score in scores {
        let mut calc = ctx.pp(&map).mode(score.mode).mods(score.mods);
        let attrs = calc.performance().await;
        let stars = attrs.stars() as f32;

        let max_pp = score
            .pp
            .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
            .unwrap_or(attrs.pp() as f32);

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        let score = ScoreSlim::new(score, pp);
        let if_fc = IfFc::new(ctx, &score, &map).await;

        let entry = CompareEntry {
            score,
            stars,
            max_pp,
            if_fc,
        };

        entries.push(entry);
    }

    match sort {
        ScoreOrder::Acc => {
            entries.sort_unstable_by(|a, b| {
                b.score
                    .accuracy
                    .partial_cmp(&a.score.accuracy)
                    .unwrap_or(Ordering::Equal)
            });
        }
        ScoreOrder::Combo => entries.sort_unstable_by_key(|s| Reverse(s.score.max_combo)),
        ScoreOrder::Date => entries.sort_unstable_by_key(|s| Reverse(s.score.ended_at)),
        ScoreOrder::Misses => entries.sort_unstable_by(|a, b| {
            b.score
                .statistics
                .count_miss
                .cmp(&a.score.statistics.count_miss)
                .then_with(|| {
                    let hits_a = a.score.total_hits();
                    let hits_b = b.score.total_hits();

                    let ratio_a = a.score.statistics.count_miss as f32 / hits_a as f32;
                    let ratio_b = b.score.statistics.count_miss as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
        }),
        ScoreOrder::Pp => {
            entries.sort_unstable_by(|a, b| {
                b.score
                    .pp
                    .partial_cmp(&a.score.pp)
                    .unwrap_or(Ordering::Equal)
            });
        }
        ScoreOrder::Score => entries.sort_unstable_by_key(|s| Reverse(s.score.score)),
        ScoreOrder::Stars => {
            entries
                .sort_unstable_by(|a, b| b.stars.partial_cmp(&a.stars).unwrap_or(Ordering::Equal));
        }
    }

    Ok(entries)
}

pub async fn handle_autocomplete(
    ctx: &Context,
    command: &InteractionCommand,
    difficulty: Option<String>,
    map: &Option<Cow<'_, str>>,
    // idx: Option<u64>, // TODO
) -> Result<()> {
    let diffs = ctx.redis().cs_diffs(command, map, None).await?;

    let diff = difficulty
        .as_deref()
        .map(CowUtils::cow_to_ascii_lowercase)
        .unwrap_or_default();

    let choices = match diffs {
        RedisData::Original(diffs) => diffs
            .into_iter()
            .filter_map(|MapVersion { map_id, version }| {
                let lowercase = version.cow_to_ascii_lowercase();

                if !lowercase.contains(&*diff) {
                    return None;
                }

                Some(CommandOptionChoice::String {
                    name: version,
                    name_localizations: None,
                    value: map_id.to_string(),
                })
            })
            .take(25)
            .collect(),
        RedisData::Archived(diffs) => diffs
            .iter()
            .filter_map(|ArchivedMapVersion { map_id, version }| {
                let lowercase = version.cow_to_ascii_lowercase();

                if !lowercase.contains(&*diff) {
                    return None;
                }

                Some(CommandOptionChoice::String {
                    name: version.as_str().to_owned(),
                    name_localizations: None,
                    value: map_id.to_string(),
                })
            })
            .take(25)
            .collect(),
    };

    command.autocomplete(ctx, choices).await?;

    Ok(())
}

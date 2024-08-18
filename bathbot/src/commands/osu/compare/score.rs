use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
};

use bathbot_macros::{command, HasMods, HasName, SlashCommand};
use bathbot_model::{
    embed_builder::{ScoreEmbedSettings, SettingsImage},
    ScoreSlim,
};
use bathbot_psql::model::{
    configs::ScoreData,
    osu::{ArchivedMapVersion, MapVersion},
};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::MapIdType,
    CowUtils, MessageOrigin,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{
        GameMode, Grade, OsuError,
        RankStatus::{self, Approved, Loved, Ranked},
        Score,
    },
    request::UserId,
};
use twilight_interactions::command::{AutocompleteValue, CommandModel, CreateCommand};
use twilight_model::{
    application::command::{CommandOptionChoice, CommandOptionChoiceValue},
    channel::message::MessageType,
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

use super::{CompareScoreAutocomplete, ScoreOrder};
use crate::{
    active::{
        impls::{CompareScoresPagination, SingleScorePagination},
        ActiveMessages,
    },
    commands::{
        osu::{map_strain_graph, require_link, HasMods, ModsResult},
        utility::{ScoreEmbedData, ScoreEmbedDataPersonalBest},
    },
    core::commands::{
        prefix::{Args, ArgsNum},
        CommandOrigin,
    },
    manager::{
        redis::{
            osu::{UserArgs, UserArgsSlim},
            RedisData,
        },
        MapError,
    },
    util::{
        interaction::InteractionCommand,
        osu::{IfFc, PersonalBestIndex},
        InteractionCommandExt,
    },
    Context,
};

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "cs",
    desc = "Compare a score",
    help = "Given a user and a map, display the user's scores on the map"
)]
#[allow(dead_code)]
pub struct Cs<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<Cow<'a, str>>,
    #[command(
        autocomplete = true,
        desc = "Specify a difficulty name of the map's mapset"
    )]
    difficulty: Option<String>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<ScoreOrder>,
    #[command(
        desc = "Filter out scores based on mods \
        (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out scores based on mods.\n\
        Mods must be given as `+mods` to require these mods to be included, \
        `+mods!` to require exactly these mods, \
        or `-mods!` to ignore scores containing any of these mods.\n\
        Examples:\n\
        - `+hd`: Remove scores that don't include `HD`\n\
        - `+hdhr!`: Only keep the `HDHR` score\n\
        - `+nm!`: Only keep the nomod score\n\
        - `-ezhd!`: Remove all scores that have either `EZ` or `HD`"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 50,
        desc = "While checking the channel history, I will choose the index-th map I can find"
    )]
    index: Option<u32>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "score",
    desc = "Compare a score",
    help = "Given a user and a map, display the user's scores on the map.\n\
    Its shorter alias is the `/cs` command."
)]
#[allow(dead_code)]
pub struct CompareScore_<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<Cow<'a, str>>,
    #[command(
        autocomplete = true,
        desc = "Specify a difficulty name of the map's mapset"
    )]
    difficulty: Option<String>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<ScoreOrder>,
    #[command(
        desc = "Filter out scores based on mods \
        (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out scores based on mods.\n\
        Mods must be given as `+mods` to require these mods to be included, \
        `+mods!` to require exactly these mods, \
        or `-mods!` to ignore scores containing any of these mods.\n\
        Examples:\n\
        - `+hd`: Remove scores that don't include `HD`\n\
        - `+hdhr!`: Only keep the `HDHR` score\n\
        - `+nm!`: Only keep the nomod score\n\
        - `-ezhd!`: Remove all scores that have either `EZ` or `HD`"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 50,
        desc = "While checking the channel history, I will choose the index-th map I can find"
    )]
    index: Option<u32>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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
    index: Option<u32>,
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
            index: match index {
                ArgsNum::Value(n) => Some(n),
                ArgsNum::Random | ArgsNum::None => None,
            },
        }
    }
}

impl<'a> TryFrom<CompareScoreAutocomplete<'a>> for CompareScoreArgs<'a> {
    type Error = &'static str;

    fn try_from(args: CompareScoreAutocomplete<'a>) -> Result<Self, Self::Error> {
        let map = if let Some(arg) = args.map {
            if let Some(id) = matcher::get_osu_map_id(&arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(&arg).map(MapIdType::Set))
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
            AutocompleteValue::None | AutocompleteValue::Focused(_) => None,
            AutocompleteValue::Completed(diff) => Some(diff),
        };

        Ok(Self {
            name: args.name,
            map,
            difficulty,
            sort: args.sort,
            mods: args.mods,
            discord: args.discord,
            index: args.index,
        })
    }
}

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
#[aliases("c", "score", "scores", "gap")]
#[group(AllModes)]
async fn prefix_compare(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let mut args = CompareScoreArgs::args(args);

    let reply = msg
        .referenced_message
        .as_deref()
        .filter(|_| msg.kind == MessageType::Reply);

    if let Some(msg) = reply {
        if let Some(id) = Context::find_map_id_in_msg(msg).await {
            args.map = Some(MapOrScore::Map(id));
        } else if let Some((mode, id)) = matcher::get_osu_score_id(&msg.content) {
            args.map = Some(MapOrScore::Score { id, mode });
        }
    }

    score(CommandOrigin::from_msg(msg, permissions), args).await
}

pub async fn slash_cs(mut command: InteractionCommand) -> Result<()> {
    let args = CompareScoreAutocomplete::from_interaction(command.input_data())?;

    slash_compare(&mut command, args).await
}

async fn slash_comparescore_(mut command: InteractionCommand) -> Result<()> {
    let args = CompareScoreAutocomplete::from_interaction(command.input_data())?;

    slash_compare(&mut command, args).await
}

pub async fn slash_compare(
    command: &mut InteractionCommand,
    args: CompareScoreAutocomplete<'_>,
) -> Result<()> {
    if let AutocompleteValue::Focused(diff) = args.difficulty {
        return handle_autocomplete(command, Some(diff), &args.map, args.index).await;
    }

    match CompareScoreArgs::try_from(args) {
        Ok(args) => score(command.into(), args).await,
        Err(content) => {
            command.error(content).await?;

            Ok(())
        }
    }
}

pub(super) async fn score(orig: CommandOrigin<'_>, args: CompareScoreArgs<'_>) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                To specify included mods, provide them e.g. as `+hrdt`.\n\
                For exact mods, provide it e.g. as `+hdhr!`.\n\
                And for excluded mods, provide it e.g. as `-hdnf!`.";

            return orig.error(content).await;
        }
    };

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
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
    let settings = config.score_embed.unwrap_or_default();

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

                return orig.error(content).await;
            }
            Some(MapOrScore::Score { id, mode }) => {
                return compare_from_score(orig, id, mode, settings, score_data).await
            }
            None => {
                let idx = match index {
                    Some(51..) => {
                        let content = "I can only go back 50 messages";

                        return orig.error(content).await;
                    }
                    Some(idx) => idx.saturating_sub(1) as usize,
                    None => 0,
                };

                let msgs = match Context::retrieve_channel_history(orig.channel_id()).await {
                    Ok(msgs) => msgs,
                    Err(_) => {
                        let content =
                            "No beatmap specified and lacking permission to search the channel \
                            history for maps.\nTry specifying a map either by url to the map, or \
                            just by map id, or give me the \"Read Message History\" permission.";

                        return orig.error(content).await;
                    }
                };

                match Context::find_map_id_in_msgs(&msgs, idx).await {
                    Some(MapIdType::Map(id)) => id,
                    None | Some(MapIdType::Set(_)) if idx == 0 => {
                        let content =
                            "No beatmap specified and none found in recent channel history.\n\
                            Try specifying a map either by url to the map, or just by map id.";

                        return orig.error(content).await;
                    }
                    None | Some(MapIdType::Set(_)) => {
                        let content = format!(
                            "No beatmap specified and none found at index {} \
                            of the recent channel history.\nTry decreasing the index or \
                            specify a map either by url to the map, or just by map id.",
                            idx + 1
                        );

                        return orig.error(content).await;
                    }
                }
            }
        }
    };

    // Retrieving the beatmap
    let map = match Context::osu_map().map(map_id, None).await {
        Ok(map) => map,
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?"
            );

            return orig.error(content).await;
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = map.mode();
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let (user_res, score_res) = match user_args {
        UserArgs::Args(args) => {
            let user_fut = Context::redis().osu_user_from_args(args);
            let score_fut = Context::osu_scores()
                .user_on_map(map_id, legacy_scores)
                .exec(args);

            tokio::join!(user_fut, score_fut)
        }
        UserArgs::User { user, mode } => {
            let args = UserArgsSlim::user_id(user.user_id).mode(mode);
            let user = RedisData::Original(*user);
            let score_res = Context::osu_scores()
                .user_on_map(map_id, legacy_scores)
                .exec(args)
                .await;

            (Ok(user), score_res)
        }
        UserArgs::Err(err) => (Err(err), Ok(Vec::new())),
    };

    let (user, mut scores) = match (user_res, score_res) {
        (Ok(user), Ok(scores)) => (user, scores),
        (Err(OsuError::NotFound), _) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(content).await;
        }
        (_, Err(OsuError::NotFound)) => {
            let content = "Beatmap was not found. Maybe unranked?";

            return orig.error(content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    let user_args = UserArgsSlim::user_id(user.user_id()).mode(mode);
    let scores_manager = Context::osu_scores();
    let pinned_fut = scores_manager
        .clone()
        .pinned(legacy_scores)
        .limit(100)
        .exec(user_args);

    let scores_manager_clone = scores_manager.clone();

    let global_fut = async {
        if matches!(
            map.status(),
            RankStatus::Ranked | RankStatus::Loved | RankStatus::Approved
        ) {
            let fut = scores_manager_clone.map_leaderboard(map_id, mode, None, 50, legacy_scores);

            Some(fut.await)
        } else {
            None
        }
    };

    let personal_fut = async {
        if matches!(
            map.status(),
            RankStatus::Ranked | RankStatus::Approved | RankStatus::Loved | RankStatus::Qualified
        ) {
            let user_args = UserArgsSlim::user_id(user.user_id()).mode(mode);
            let fut = scores_manager.top(legacy_scores).limit(100).exec(user_args);

            Some(fut.await)
        } else {
            None
        }
    };

    let (pinned_res, global_res, personal_res) = tokio::join!(pinned_fut, global_fut, personal_fut);

    let pinned = match pinned_res {
        Ok(scores) => scores,
        Err(err) => {
            warn!(?err, "Failed to get pinned scores");

            Vec::new()
        }
    };

    let globals = match global_res {
        Some(Ok(globals)) => Some(globals),
        Some(Err(err)) => {
            warn!(?err, "Failed to get map leaderboard");

            None
        }
        None => None,
    };

    let personal = match personal_res {
        Some(Ok(scores)) => Some(scores.into_boxed_slice()),
        Some(Err(err)) => {
            warn!(?err, "Failed to get top scores");

            None
        }
        None => None,
    };

    if let Some(ref selection) = mods {
        selection.filter_scores(&mut scores);
    }

    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());

    let process_fut = process_scores(
        map_id,
        user.user_id(),
        scores,
        personal.as_deref(),
        globals.as_deref(),
        sort.unwrap_or_default(),
        score_data,
        &origin,
    );

    let entries = match process_fut.await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to process scores"));
        }
    };

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

    let graph = match entries.first() {
        Some(entry) if matches!(settings.image, SettingsImage::ImageWithStrains) => {
            prepare_graph(entry).await
        }
        Some(_) | None => None,
    };

    let pagination = CompareScoresPagination::builder()
        .user(user)
        .map(map)
        .settings(settings)
        .entries(entries)
        .pinned(pinned.into_boxed_slice())
        .pp_idx(pp_idx)
        .score_data(score_data)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .attachment(graph)
        .begin(orig)
        .await
}

#[allow(clippy::too_many_arguments)]
async fn process_scores(
    map_id: u32,
    user_id: u32,
    scores: Vec<Score>,
    top100: Option<&[Score]>,
    globals: Option<&[Score]>,
    sort: ScoreOrder,
    score_data: ScoreData,
    origin: &MessageOrigin,
) -> Result<Box<[ScoreEmbedData]>> {
    let map = Context::osu_map().map(map_id, None).await?;

    let mut entries = Vec::<ScoreEmbedData>::with_capacity(scores.len());

    for score in scores {
        let mut calc = Context::pp(&map).mode(score.mode).mods(&score.mods);
        let attrs = calc.performance().await;
        let stars = attrs.stars() as f32;
        let max_combo = attrs.max_combo();

        let max_pp = score
            .pp
            .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
            .unwrap_or(attrs.pp() as f32);

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        let score = ScoreSlim::new(score, pp);
        let if_fc_pp = IfFc::new(&score, &map).await.map(|if_fc| if_fc.pp);

        let pb_idx = top100.and_then(|top100| {
            let pb_idx = PersonalBestIndex::new(&score, map_id, map.status(), top100);

            ScoreEmbedDataPersonalBest::try_new(pb_idx, origin)
        });

        let global_idx = globals.and_then(|globals| {
            globals
                .iter()
                .position(|s| s.user_id == user_id && score.is_eq(s))
                .map(|idx| idx + 1)
        });

        let entry = ScoreEmbedData {
            score,
            map: map.clone(),
            stars,
            max_combo,
            max_pp,
            replay: None,
            miss_analyzer: None,
            pb_idx,
            global_idx,
            if_fc_pp,
            #[cfg(feature = "twitch")]
            twitch: None,
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
        ScoreOrder::Score if score_data == ScoreData::LazerWithClassicScoring => {
            entries.sort_unstable_by_key(|s| Reverse(s.score.classic_score))
        }
        ScoreOrder::Score => entries.sort_unstable_by_key(|s| Reverse(s.score.score)),
        ScoreOrder::Stars => {
            entries
                .sort_unstable_by(|a, b| b.stars.partial_cmp(&a.stars).unwrap_or(Ordering::Equal));
        }
    }

    Ok(entries.into_boxed_slice())
}

async fn compare_from_score(
    orig: CommandOrigin<'_>,
    score_id: u64,
    mode: GameMode,
    settings: ScoreEmbedSettings,
    score_data: ScoreData,
) -> Result<()> {
    let mut score = match Context::osu().score(score_id).mode(mode).await {
        Ok(score) => score,
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get score");

            return Err(err);
        }
    };

    let legacy_scores = score_data.is_legacy();

    let map = score.map.take().expect("missing map");
    let map_fut = Context::osu_map().map(map.map_id, map.checksum.as_deref());

    let user_args = UserArgs::user_id(score.user_id, mode);
    let user_fut = Context::redis().osu_user(user_args);

    let user_args = UserArgsSlim::user_id(score.user_id).mode(mode);
    let scores_manager = Context::osu_scores();
    let pinned_fut = scores_manager
        .clone()
        .pinned(legacy_scores)
        .limit(100)
        .exec(user_args);

    let (user_res, map_res, pinned_res) = tokio::join!(user_fut, map_fut, pinned_fut);

    let user = match user_res {
        Ok(user) => user,
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let user_id = user.user_id();

    let map = match map_res {
        Ok(map) => map,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err));
        }
    };

    let pinned = match pinned_res {
        Ok(scores) => scores,
        Err(err) => {
            warn!(?err, "Failed to get pinned scores");

            Vec::new()
        }
    };

    let scores_manager_clone = scores_manager.clone();

    let globals = if matches!(map.status(), Ranked | Loved | Approved) {
        let fut = scores_manager_clone.map_leaderboard(map.map_id(), mode, None, 50, legacy_scores);

        match fut.await {
            Ok(globals) => Some(globals),
            Err(err) => {
                warn!(?err, "Failed to get global scores");

                None
            }
        }
    } else {
        None
    };

    let top100 = if map.status() == Ranked {
        let user_args = UserArgsSlim::user_id(user_id).mode(mode);
        let fut = scores_manager.top(legacy_scores).limit(100).exec(user_args);

        match fut.await {
            Ok(scores) => Some(scores.into_boxed_slice()),
            Err(err) => {
                warn!(?err, "Failed to get top scores");

                None
            }
        }
    } else {
        None
    };

    let mut calc = Context::pp(&map).mode(score.mode).mods(&score.mods);
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
    let if_fc_pp = IfFc::new(&score, &map).await.map(|if_fc| if_fc.pp);
    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());

    let pb_idx = top100.as_deref().and_then(|top100| {
        let pb_idx = PersonalBestIndex::new(&score, map.map_id(), map.status(), top100);

        ScoreEmbedDataPersonalBest::try_new(pb_idx, &origin)
    });

    let global_idx = globals.and_then(|globals| {
        globals
            .iter()
            .position(|s| s.user_id == user_id && score.is_eq(s))
            .map(|idx| idx + 1)
    });

    let entry = ScoreEmbedData {
        score,
        map: map.clone(),
        stars: attrs.stars() as f32,
        max_combo: attrs.max_combo(),
        max_pp,
        replay: None,
        miss_analyzer: None,
        pb_idx,
        global_idx,
        if_fc_pp,
        #[cfg(feature = "twitch")]
        twitch: None,
    };

    let graph = if matches!(settings.image, SettingsImage::ImageWithStrains) {
        prepare_graph(&entry).await
    } else {
        None
    };

    let pagination = CompareScoresPagination::builder()
        .user(user)
        .map(map)
        .settings(settings)
        .entries(Box::from([entry]))
        .pinned(pinned.into_boxed_slice())
        .pp_idx(0)
        .score_data(score_data)
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .attachment(graph)
        .begin(orig)
        .await
}

async fn handle_autocomplete(
    command: &InteractionCommand,
    difficulty: Option<String>,
    map: &Option<Cow<'_, str>>,
    idx: Option<u32>,
) -> Result<()> {
    let diffs = Context::redis().cs_diffs(command, map, idx).await?;

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

                Some(CommandOptionChoice {
                    name: version,
                    name_localizations: None,
                    // Discord requires these as strings
                    value: CommandOptionChoiceValue::String(map_id.to_string()),
                })
            })
            .take(25)
            .collect(),
        RedisData::Archive(diffs) => diffs
            .iter()
            .filter_map(|ArchivedMapVersion { map_id, version }| {
                let lowercase = version.cow_to_ascii_lowercase();

                if !lowercase.contains(&*diff) {
                    return None;
                }

                Some(CommandOptionChoice {
                    name: version.as_str().to_owned(),
                    name_localizations: None,
                    // Discord requires these as strings
                    value: CommandOptionChoiceValue::String(map_id.to_string()),
                })
            })
            .take(25)
            .collect(),
    };

    command.autocomplete(choices).await?;

    Ok(())
}

async fn prepare_graph(entry: &ScoreEmbedData) -> Option<(String, Vec<u8>)> {
    let fut = map_strain_graph(
        &entry.map.pp_map,
        entry.score.mods.clone(),
        entry.map.cover(),
    );

    match fut.await {
        Ok(graph) => Some((SingleScorePagination::IMAGE_NAME.to_owned(), graph)),
        Err(err) => {
            warn!(?err, "Failed to create strain graph");

            None
        }
    }
}

use std::sync::Arc;

use eyre::Report;
use rosu_pp::{
    fruits::stars, Beatmap as Map, FruitsPP, ManiaPP, OsuPP, PerformanceAttributes, TaikoPP,
};
use rosu_v2::prelude::{Beatmap, GameMode, GameMods, OsuError, RankStatus, Score, User};
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    channel::message::MessageType,
    id::UserId,
};

use crate::{
    commands::{check_user_mention, parse_discord, DoubleResultCow, MyCommand},
    database::OsuData,
    embeds::{EmbedData, FixScoreEmbed},
    error::{Error, PPError},
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MAP, MAP_PARSE_FAIL, MODS, MODS_PARSE_FAIL, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher,
        osu::{
            map_id_from_history, map_id_from_msg, prepare_beatmap_file, MapIdType, ModSelection,
        },
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

use super::{
    get_beatmap_user_score, get_user, option_discord, option_map, option_mods, option_name,
    UserArgs,
};

#[command]
#[short_desc("Display a user's pp after unchoking their score on a map")]
#[long_desc(
    "Display a user's pp after unchoking their score on a map. \n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified but only if there already is a score \
     on the map with those mods."
)]
#[aliases("fixscore")]
#[usage("[username] [map url / map id] [+mods]")]
#[example(
    "badewanne3",
    "badewanne3 2240404 +hdhr",
    "https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
async fn fix(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match FixArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut fix_args)) => {
                    let reply = msg
                        .referenced_message
                        .as_ref()
                        .filter(|_| msg.kind == MessageType::Reply);

                    if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                        fix_args.id = Some(MapOrScore::Map(id));
                    } else if let Some((mode, id)) =
                        reply.and_then(|msg| matcher::get_osu_score_id(&msg.content))
                    {
                        fix_args.id = Some(MapOrScore::Score { id, mode });
                    }

                    _fix(ctx, CommandData::Message { msg, args, num }, fix_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => slash_fix(ctx, *command).await,
    }
}

async fn _fix(ctx: Arc<Context>, data: CommandData<'_>, args: FixArgs) -> BotResult<()> {
    let FixArgs { osu, id, mods } = args;

    let name = match osu.map(OsuData::into_username) {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let mods = match mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) => Some(mods),
    };

    let data_result = match id {
        Some(MapOrScore::Score { id, mode }) => {
            request_by_score(&ctx, &data, id, mode, name.as_str()).await
        }
        Some(MapOrScore::Map(MapIdType::Map(id))) => {
            request_by_map(&ctx, &data, id, name.as_str(), mods).await
        }
        Some(MapOrScore::Map(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return data.error(&ctx, content).await;
        }
        None => {
            let msgs = match ctx.retrieve_channel_history(data.channel_id()).await {
                Ok(msgs) => msgs,
                Err(why) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(why);
                }
            };

            match map_id_from_history(&msgs) {
                Some(MapIdType::Map(id)) => {
                    request_by_map(&ctx, &data, id, name.as_str(), mods).await
                }
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
            }
        }
    };

    let ScoreData {
        user,
        map,
        mut scores,
    } = match data_result {
        ScoreResult::Data(data) => data,
        ScoreResult::Done => return Ok(()),
        ScoreResult::Error(err) => return Err(err),
    };

    if map.mode == GameMode::MNA {
        return data.error(&ctx, "Can't fix mania scores \\:(").await;
    }

    let unchoked_pp = match scores {
        Some((ref mut score, _)) => {
            if score.pp.is_some() && !needs_unchoking(score, &map) {
                None
            } else {
                match unchoke_pp(score, &map).await {
                    Ok(pp) => pp,
                    Err(why) => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                }
            }
        }
        None => None,
    };

    // Process tracking
    if let Some((_, best)) = scores.as_mut().filter(|_| {
        unchoked_pp.is_some() || matches!(map.status, RankStatus::Ranked | RankStatus::Approved)
    }) {
        process_tracking(&ctx, best, Some(&user)).await;
    }

    let gb = ctx.map_garbage_collector(&map);

    let embed_data = FixScoreEmbed::new(user, map, scores, unchoked_pp, mods);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    // Set map on garbage collection list if unranked
    gb.execute(&ctx).await;

    Ok(())
}

#[allow(clippy::large_enum_variant)]
enum ScoreResult {
    Data(ScoreData),
    Done,
    Error(Error),
}

struct ScoreData {
    user: User,
    map: Beatmap,
    scores: Option<(Score, Vec<Score>)>,
}

// Retrieve user's score on the map, the user itself, and the map including mapset
async fn request_by_map(
    ctx: &Context,
    data: &CommandData<'_>,
    map_id: u32,
    name: &str,
    mods: Option<GameMods>,
) -> ScoreResult {
    let user_args = UserArgs::new(name, GameMode::STD);

    match get_beatmap_user_score(ctx.osu(), map_id, &user_args, mods).await {
        Ok(mut score) => match super::prepare_score(ctx, &mut score.score).await {
            Ok(_) => {
                let mut map = score.score.map.take().unwrap();

                // First try to just get the mapset from the DB
                let mapset_fut = ctx.psql().get_beatmapset(map.mapset_id);
                let user_fut = ctx.osu().user(score.score.user_id).mode(score.score.mode);

                let best_fut = ctx
                    .osu()
                    .user_scores(score.score.user_id)
                    .mode(score.score.mode)
                    .limit(100)
                    .best();

                let (user, best) = match tokio::join!(mapset_fut, user_fut, best_fut) {
                    (_, Err(why), _) | (_, _, Err(why)) => {
                        let _ = data.error(ctx, OSU_API_ISSUE).await;

                        return ScoreResult::Error(why.into());
                    }
                    (Ok(mapset), Ok(user), Ok(best)) => {
                        map.mapset = Some(mapset);

                        (user, best)
                    }
                    (Err(_), Ok(user), Ok(best)) => {
                        let mapset = match ctx.osu().beatmapset(map.mapset_id).await {
                            Ok(mapset) => mapset,
                            Err(why) => {
                                let _ = data.error(ctx, OSU_API_ISSUE).await;

                                return ScoreResult::Error(why.into());
                            }
                        };

                        map.mapset = Some(mapset);

                        (user, best)
                    }
                };

                let data = ScoreData {
                    user,
                    map,
                    scores: Some((score.score, best)),
                };

                ScoreResult::Data(data)
            }
            Err(why) => {
                let _ = data.error(ctx, OSU_API_ISSUE).await;

                ScoreResult::Error(why.into())
            }
        },
        // Either the user, map, or user score on the map don't exist
        Err(OsuError::NotFound) => {
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
                        let content = format!("There is no map with id {}", map_id);

                        return match data.error(ctx, content).await {
                            Ok(_) => ScoreResult::Done,
                            Err(err) => ScoreResult::Error(err),
                        };
                    }
                    Err(why) => {
                        let _ = data.error(ctx, OSU_API_ISSUE).await;

                        return ScoreResult::Error(why.into());
                    }
                },
            };

            let user_args = UserArgs::new(name, map.mode);

            let user = match get_user(ctx, &user_args).await {
                Ok(user) => user,
                Err(OsuError::NotFound) => {
                    let content = format!("Could not find user `{}`", name);

                    return match data.error(ctx, content).await {
                        Ok(_) => ScoreResult::Done,
                        Err(err) => ScoreResult::Error(err),
                    };
                }
                Err(why) => {
                    let _ = data.error(ctx, OSU_API_ISSUE).await;

                    return ScoreResult::Error(why.into());
                }
            };

            let data = ScoreData {
                user,
                map,
                scores: None,
            };

            ScoreResult::Data(data)
        }
        Err(why) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            ScoreResult::Error(why.into())
        }
    }
}

async fn request_by_score(
    ctx: &Context,
    data: &CommandData<'_>,
    score_id: u64,
    mode: GameMode,
    name: &str,
) -> ScoreResult {
    let score_fut = ctx.osu().score(score_id, mode);
    let user_fut = ctx.osu().user(name).mode(mode);

    let (user, mut score) = match tokio::try_join!(user_fut, score_fut) {
        Ok((user, score)) => (user, score),
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return ScoreResult::Error(err.into());
        }
    };

    let mut map = score.map.take().unwrap();

    // First try to just get the mapset from the DB
    let mapset_fut = ctx.psql().get_beatmapset(map.mapset_id);

    let best_fut = ctx
        .osu()
        .user_scores(score.user_id)
        .mode(score.mode)
        .limit(100)
        .best();

    let best = match tokio::join!(mapset_fut, best_fut) {
        (_, Err(why)) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return ScoreResult::Error(why.into());
        }
        (Ok(mapset), Ok(best)) => {
            map.mapset = Some(mapset);

            best
        }
        (Err(_), Ok(best)) => {
            let mapset = match ctx.osu().beatmapset(map.mapset_id).await {
                Ok(mapset) => mapset,
                Err(why) => {
                    let _ = data.error(ctx, OSU_API_ISSUE).await;

                    return ScoreResult::Error(why.into());
                }
            };

            map.mapset = Some(mapset);

            best
        }
    };

    let data = ScoreData {
        user,
        map,
        scores: Some((score, best)),
    };

    ScoreResult::Data(data)
}

/// Returns (actual pp, unchoked pp) tuple
async fn unchoke_pp(score: &mut Score, map: &Beatmap) -> BotResult<Option<f32>> {
    let map_path = prepare_beatmap_file(map.map_id).await?;
    let rosu_map = Map::from_path(map_path).await.map_err(PPError::from)?;
    let mods = score.mods.bits();

    let attributes = if score.pp.is_some() {
        None
    } else {
        let pp_result: PerformanceAttributes = match map.mode {
            GameMode::STD => OsuPP::new(&rosu_map)
                .mods(mods)
                .combo(score.max_combo as usize)
                .n300(score.statistics.count_300 as usize)
                .n100(score.statistics.count_100 as usize)
                .n50(score.statistics.count_50 as usize)
                .misses(score.statistics.count_miss as usize)
                .calculate()
                .into(),
            GameMode::MNA => ManiaPP::new(&rosu_map)
                .mods(mods)
                .score(score.score)
                .calculate()
                .into(),
            GameMode::CTB => FruitsPP::new(&rosu_map)
                .mods(mods)
                .combo(score.max_combo as usize)
                .fruits(score.statistics.count_300 as usize)
                .droplets(score.statistics.count_100 as usize)
                .misses(score.statistics.count_miss as usize)
                .accuracy(score.accuracy as f64)
                .calculate()
                .into(),
            GameMode::TKO => TaikoPP::new(&rosu_map)
                .combo(score.max_combo as usize)
                .mods(mods)
                .misses(score.statistics.count_miss as usize)
                .accuracy(score.accuracy as f64)
                .calculate()
                .into(),
        };

        score.pp.replace(pp_result.pp() as f32);

        if !needs_unchoking(score, map) {
            return Ok(None);
        }

        Some(pp_result)
    };

    let unchoked_pp = match map.mode {
        GameMode::STD => {
            let total_objects = map.count_objects() as usize;

            let mut count300 = score.statistics.count_300 as usize;

            let count_hits = total_objects - score.statistics.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

            count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.statistics.count_100 + new100s) as usize;
            let count50 = score.statistics.count_50 as usize;

            let mut calculator = OsuPP::new(&rosu_map);

            if let Some(attributes) = attributes {
                calculator = calculator.attributes(attributes);
            }

            calculator
                .mods(mods)
                .n300(count300)
                .n100(count100)
                .n50(count50)
                .calculate()
                .pp
        }
        GameMode::CTB => {
            let attributes = match attributes {
                Some(PerformanceAttributes::Fruits(attrs)) => attrs.difficulty,
                Some(_) => panic!("no ctb attributes after calculating stars for ctb map"),
                None => stars(&rosu_map, mods, None),
            };

            let total_objects = attributes.max_combo();
            let passed_objects = (score.statistics.count_300
                + score.statistics.count_100
                + score.statistics.count_miss) as usize;

            let missing = total_objects.saturating_sub(passed_objects);
            let missing_fruits = missing.saturating_sub(
                attributes
                    .n_droplets
                    .saturating_sub(score.statistics.count_100 as usize),
            );
            let missing_droplets = missing - missing_fruits;

            let n_fruits = score.statistics.count_300 as usize + missing_fruits;
            let n_droplets = score.statistics.count_100 as usize + missing_droplets;
            let n_tiny_droplet_misses = score.statistics.count_katu as usize;
            let n_tiny_droplets = score.statistics.count_50 as usize;

            FruitsPP::new(&rosu_map)
                .attributes(attributes)
                .mods(mods)
                .fruits(n_fruits)
                .droplets(n_droplets)
                .tiny_droplets(n_tiny_droplets)
                .tiny_droplet_misses(n_tiny_droplet_misses)
                .calculate()
                .pp
        }
        GameMode::TKO => {
            let total_objects = map.count_circles as usize;
            let passed_objects = score.total_hits() as usize;

            let mut count300 =
                score.statistics.count_300 as usize + total_objects.saturating_sub(passed_objects);

            let count_hits = total_objects - score.statistics.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

            count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.statistics.count_100 + new100s) as usize;

            let acc = 100.0 * (2 * count300 + count100) as f32 / (2 * total_objects) as f32;

            let mut calculator = TaikoPP::new(&rosu_map);

            if let Some(attributes) = attributes {
                calculator = calculator.attributes(attributes);
            }

            calculator.mods(mods).accuracy(acc as f64).calculate().pp
        }
        GameMode::MNA => panic!("can not unchoke mania scores"),
    };

    Ok(Some(unchoked_pp as f32))
}

fn needs_unchoking(score: &Score, map: &Beatmap) -> bool {
    match map.mode {
        GameMode::STD => {
            score.statistics.count_miss > 0
                || score.max_combo < map.max_combo.map_or(0, |c| c.saturating_sub(5))
        }
        GameMode::TKO => score.statistics.count_miss > 0,
        GameMode::CTB => score.max_combo != map.max_combo.unwrap_or(0),
        GameMode::MNA => panic!("can not unchoke mania scores"),
    }
}

enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

struct FixArgs {
    osu: Option<OsuData>,
    id: Option<MapOrScore>,
    mods: Option<ModSelection>,
}

impl FixArgs {
    async fn args(ctx: &Context, args: &mut Args<'_>, author_id: UserId) -> DoubleResultCow<Self> {
        let mut osu = ctx.psql().get_user_osu(author_id).await?;
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
                    Ok(osu_) => osu = Some(osu_),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        Ok(Ok(Self { osu, id, mods }))
    }

    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let mut osu = ctx.psql().get_user_osu(command.user_id()?).await?;
        let mut id = None;
        let mut mods = None;

        for option in command.yoink_options() {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => osu = Some(value.into()),
                    MAP => match matcher::get_osu_map_id(&value)
                        .or_else(|| matcher::get_osu_mapset_id(&value))
                    {
                        Some(id_) => id = Some(MapOrScore::Map(id_)),
                        None => match matcher::get_osu_score_id(&value) {
                            Some((mode, id_)) => id = Some(MapOrScore::Score { mode, id: id_ }),
                            None => return Ok(Err(MAP_PARSE_FAIL.into())),
                        },
                    },
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => match value.parse() {
                            Ok(mods_) => mods = Some(ModSelection::Exact(mods_)),
                            Err(_) => return Ok(Err(MODS_PARSE_FAIL.into())),
                        },
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu_) => osu = Some(osu_),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        Ok(Ok(Self { osu, id, mods }))
    }
}

pub async fn slash_fix(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match FixArgs::slash(&ctx, &mut command).await? {
        Ok(args) => _fix(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_fix() -> MyCommand {
    let name = option_name();
    let map = option_map();
    let mods = option_mods(false);
    let discord = option_discord();

    let description = "Display a user's pp after unchoking their score on a map";

    MyCommand::new("fix", description).options(vec![name, map, mods, discord])
}

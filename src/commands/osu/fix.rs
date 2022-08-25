use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasMods, HasName, SlashCommand};
use eyre::Report;
use rosu_pp::{
    Beatmap as Map, CatchPP, CatchStars, ManiaPP, OsuPP, PerformanceAttributes, TaikoPP,
};
use rosu_v2::prelude::{Beatmap, GameMode, GameMods, OsuError, Score, User};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{message::MessageType, Message},
    id::{marker::UserMarker, Id},
};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, FixScoreEmbed},
    error::{Error, PpError},
    tracking::process_osu_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        matcher,
        osu::{prepare_beatmap_file, MapIdType, ModSelection},
        InteractionCommandExt, ScoreExt,
    },
    BotResult, Context,
};

use super::{get_beatmap_user_score, get_user, require_link, HasMods, ModsResult, UserArgs};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "fix")]
/// Display a user's pp after unchoking their score on a map
pub struct Fix<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.\
        Alternatively, you can also provide a score url.")]
    /// Specify a map url or map id
    map: Option<String>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(HasMods, HasName)]
struct FixArgs<'a> {
    name: Option<Cow<'a, str>>,
    id: Option<MapOrScore>,
    mods: Option<Cow<'a, str>>,
    discord: Option<Id<UserMarker>>,
}

enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

impl<'m> FixArgs<'m> {
    fn args(msg: &Message, args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;
        let mut id_ = None;
        let mut mods = None;

        for arg in args.take(3) {
            if let Some(id) = matcher::get_osu_map_id(arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set))
            {
                id_ = Some(MapOrScore::Map(id));
            } else if let Some((mode, id)) = matcher::get_osu_score_id(arg) {
                id_ = Some(MapOrScore::Score { mode, id });
            } else if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        let reply = msg
            .referenced_message
            .as_deref()
            .filter(|_| msg.kind == MessageType::Reply);

        if let Some(reply) = reply {
            if let Some(id) = MapIdType::from_msg(reply) {
                id_ = Some(MapOrScore::Map(id));
            } else if let Some((mode, id)) = matcher::get_osu_score_id(&reply.content) {
                id_ = Some(MapOrScore::Score { mode, id });
            }
        }

        Self {
            name,
            discord,
            id: id_,
            mods,
        }
    }
}

impl<'a> TryFrom<Fix<'a>> for FixArgs<'a> {
    type Error = &'static str;

    fn try_from(args: Fix<'a>) -> Result<Self, Self::Error> {
        let id = match args.map {
            Some(map) => {
                if let Some(id) = matcher::get_osu_map_id(&map)
                    .map(MapIdType::Map)
                    .or_else(|| matcher::get_osu_mapset_id(&map).map(MapIdType::Set))
                {
                    Some(MapOrScore::Map(id))
                } else if let Some((mode, id)) = matcher::get_osu_score_id(&map) {
                    Some(MapOrScore::Score { mode, id })
                } else {
                    return Err(
                        "Failed to parse map url. Be sure you specify a valid map id or url to a map.",
                    );
                }
            }
            None => None,
        };

        Ok(Self {
            name: args.name,
            id,
            mods: args.mods,
            discord: args.discord,
        })
    }
}

async fn slash_fix(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    let args = Fix::from_interaction(command.input_data())?;

    match FixArgs::try_from(args) {
        Ok(args) => fix(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's pp after unchoking their score on a map")]
#[help(
    "Display a user's pp after unchoking their score on a map. \n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified but only if there already is a score \
     on the map with those mods."
)]
#[alias("fixscore")]
#[usage("[username] [map url / map id] [+mods]")]
#[examples(
    "badewanne3",
    "badewanne3 2240404 +hdhr",
    "https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
#[group(AllModes)]
async fn prefix_fix(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    let args = FixArgs::args(msg, args);

    fix(ctx, msg.into(), args).await
}

async fn fix(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: FixArgs<'_>) -> BotResult<()> {
    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(orig.user_id()?).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods. Be sure to either specify them directly \
            or through the `+mods` / `+mods!` syntax e.g. `hdhr` or `+hdhr!`";

            return orig.error(&ctx, content).await;
        }
    };

    let mods = match mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) => Some(mods),
    };

    let data_result = match args.id {
        Some(MapOrScore::Score { id, mode }) => {
            request_by_score(&ctx, &orig, id, mode, name.as_str()).await
        }
        Some(MapOrScore::Map(MapIdType::Map(id))) => {
            request_by_map(&ctx, &orig, id, name.as_str(), mods).await
        }
        Some(MapOrScore::Map(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(&ctx, content).await;
        }
        None => {
            let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            };

            match MapIdType::map_from_msgs(&msgs, 0) {
                Some(id) => request_by_map(&ctx, &orig, id, name.as_str(), mods).await,
                None => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return orig.error(&ctx, content).await;
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

    if map.mode == GameMode::Mania {
        return orig.error(&ctx, "Can't fix mania scores \\:(").await;
    }

    let unchoked_pp = match scores {
        Some((ref mut score, _)) => {
            if score.pp.is_some() && !needs_unchoking(score, &map) {
                None
            } else {
                match unchoke_pp(&ctx, score, &map).await {
                    Ok(pp) => pp,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err);
                    }
                }
            }
        }
        None => None,
    };

    // Process tracking
    if let Some((_, best)) = scores.as_mut() {
        process_osu_tracking(&ctx, best, Some(&user)).await;
    }

    let gb = ctx.map_garbage_collector(&map);

    let embed_data = FixScoreEmbed::new(user, map, scores, unchoked_pp, mods);
    let builder = embed_data.build().into();
    orig.create_message(&ctx, &builder).await?;

    // Set map on garbage collection list if unranked
    gb.execute(&ctx);

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
    orig: &CommandOrigin<'_>,
    map_id: u32,
    name: &str,
    mods: Option<GameMods>,
) -> ScoreResult {
    let user_args = UserArgs::new(name, GameMode::Osu);

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
                    (_, Err(err), _) | (_, _, Err(err)) => {
                        let _ = orig.error(ctx, OSU_API_ISSUE).await;

                        return ScoreResult::Error(err.into());
                    }
                    (Ok(mapset), Ok(user), Ok(best)) => {
                        map.mapset = Some(mapset);

                        (user, best)
                    }
                    (Err(_), Ok(user), Ok(best)) => {
                        let mapset = match ctx.osu().beatmapset(map.mapset_id).await {
                            Ok(mapset) => mapset,
                            Err(err) => {
                                let _ = orig.error(ctx, OSU_API_ISSUE).await;

                                return ScoreResult::Error(err.into());
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
            Err(err) => {
                let _ = orig.error(ctx, OSU_API_ISSUE).await;

                ScoreResult::Error(err.into())
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
                        let content = format!("There is no map with id {map_id}");

                        return match orig.error(ctx, content).await {
                            Ok(_) => ScoreResult::Done,
                            Err(err) => ScoreResult::Error(err),
                        };
                    }
                    Err(err) => {
                        let _ = orig.error(ctx, OSU_API_ISSUE).await;

                        return ScoreResult::Error(err.into());
                    }
                },
            };

            let user_args = UserArgs::new(name, map.mode);

            let user = match get_user(ctx, &user_args).await {
                Ok(user) => user,
                Err(OsuError::NotFound) => {
                    let content = format!("Could not find user `{name}`");

                    return match orig.error(ctx, content).await {
                        Ok(_) => ScoreResult::Done,
                        Err(err) => ScoreResult::Error(err),
                    };
                }
                Err(err) => {
                    let _ = orig.error(ctx, OSU_API_ISSUE).await;

                    return ScoreResult::Error(err.into());
                }
            };

            let data = ScoreData {
                user,
                map,
                scores: None,
            };

            ScoreResult::Data(data)
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;

            ScoreResult::Error(err.into())
        }
    }
}

async fn request_by_score(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    score_id: u64,
    mode: GameMode,
    name: &str,
) -> ScoreResult {
    let score_fut = ctx.osu().score(score_id, mode);
    let user_fut = ctx.osu().user(name).mode(mode);

    let (user, mut score) = match tokio::try_join!(user_fut, score_fut) {
        Ok((user, score)) => (user, score),
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;

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
        (_, Err(err)) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;

            return ScoreResult::Error(err.into());
        }
        (Ok(mapset), Ok(best)) => {
            map.mapset = Some(mapset);

            best
        }
        (Err(_), Ok(best)) => {
            let mapset = match ctx.osu().beatmapset(map.mapset_id).await {
                Ok(mapset) => mapset,
                Err(err) => {
                    let _ = orig.error(ctx, OSU_API_ISSUE).await;

                    return ScoreResult::Error(err.into());
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

/// Returns unchoked pp and sets score pp if not available already
pub(super) async fn unchoke_pp(
    ctx: &Context,
    score: &mut Score,
    map: &Beatmap,
) -> BotResult<Option<f32>> {
    let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
    let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;
    let mods = score.mods.bits();

    let attributes = if score.pp.is_some() {
        None
    } else {
        let pp_result: PerformanceAttributes = match map.mode {
            GameMode::Osu => OsuPP::new(&rosu_map)
                .mods(mods)
                .combo(score.max_combo as usize)
                .n300(score.statistics.count_300 as usize)
                .n100(score.statistics.count_100 as usize)
                .n50(score.statistics.count_50 as usize)
                .misses(score.statistics.count_miss as usize)
                .calculate()
                .into(),
            GameMode::Mania => ManiaPP::new(&rosu_map)
                .mods(mods)
                .score(score.score)
                .calculate()
                .into(),
            GameMode::Catch => CatchPP::new(&rosu_map)
                .mods(mods)
                .combo(score.max_combo as usize)
                .fruits(score.statistics.count_300 as usize)
                .droplets(score.statistics.count_100 as usize)
                .misses(score.statistics.count_miss as usize)
                .accuracy(score.accuracy as f64)
                .calculate()
                .into(),
            GameMode::Taiko => TaikoPP::new(&rosu_map)
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
        GameMode::Osu => {
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
        GameMode::Catch => {
            let attributes = match attributes {
                Some(PerformanceAttributes::Catch(attrs)) => attrs.difficulty,
                Some(_) => panic!("no ctb attributes after calculating stars for ctb map"),
                None => CatchStars::new(&rosu_map).mods(mods).calculate(),
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

            CatchPP::new(&rosu_map)
                .attributes(attributes)
                .mods(mods)
                .fruits(n_fruits)
                .droplets(n_droplets)
                .tiny_droplets(n_tiny_droplets)
                .tiny_droplet_misses(n_tiny_droplet_misses)
                .calculate()
                .pp
        }
        GameMode::Taiko => {
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
        GameMode::Mania => panic!("can not unchoke mania scores"),
    };

    Ok(Some(unchoked_pp as f32))
}

fn needs_unchoking(score: &Score, map: &Beatmap) -> bool {
    !score.is_fc(map.mode, map.max_combo.unwrap_or(0))
}

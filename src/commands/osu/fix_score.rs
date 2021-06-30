use super::{prepare_score, request_user};
use crate::{
    arguments::{Args, NameMapModArgs},
    embeds::{EmbedData, FixScoreEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        error::PPError,
        osu::{
            cached_message_extract, map_id_from_history, map_id_from_msg, prepare_beatmap_file,
            MapIdType, ModSelection,
        },
        MessageExt,
    },
    BotResult, Context,
};

use rosu_pp::{fruits::stars, Beatmap as Map, FruitsPP, ManiaPP, OsuPP, StarResult, TaikoPP};
use rosu_v2::prelude::{Beatmap, GameMode, OsuError, RankStatus, Score};
use std::sync::Arc;
use tokio::fs::File;
use twilight_model::channel::{message::MessageType, Message};

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
async fn fix(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameMapModArgs::new(&ctx, args);

    let map_id_opt = args
        .map_id
        .or_else(|| {
            msg.referenced_message
                .as_ref()
                .filter(|_| msg.kind == MessageType::Reply)
                .and_then(|msg| map_id_from_msg(msg))
        })
        .or_else(|| {
            ctx.cache
                .message_extract(msg.channel_id, cached_message_extract)
        });

    let map_id = if let Some(id) = map_id_opt {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(msg.channel_id).await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
        };

        match map_id_from_history(&msgs) {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                return msg.error(&ctx, content).await;
            }
        }
    };

    let map_id = match map_id {
        MapIdType::Map(id) => id,
        MapIdType::Set(_) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return msg.error(&ctx, content).await;
        }
    };

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let arg_mods = match args.mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) => Some(mods),
    };

    let score_fut = ctx.osu().beatmap_user_score(map_id, name.as_str());

    let score_fut = match arg_mods {
        None => score_fut,
        Some(mods) => score_fut.mods(mods),
    };

    // Retrieve user's score on the map, the user itself, and the map including mapset
    let (user, map, mut scores) = match score_fut.await {
        Ok(mut score) => match prepare_score(&ctx, &mut score.score).await {
            Ok(_) => {
                let mut score = score.score;
                let mut map = score.map.take().unwrap();
                let mapset_id = map.mapset_id;

                // First try to just get the mapset from the DB
                let mapset_fut = ctx.psql().get_beatmapset(mapset_id);
                let user_fut = ctx.osu().user(score.user_id).mode(score.mode);
                let best_fut = ctx
                    .osu()
                    .user_scores(score.user_id)
                    .mode(score.mode)
                    .limit(50)
                    .best();

                let (user, best) = match tokio::join!(mapset_fut, user_fut, best_fut) {
                    (_, Err(why), _) | (_, _, Err(why)) => {
                        let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                        return Err(why.into());
                    }
                    (Ok(mapset), Ok(user), Ok(best)) => {
                        map.mapset.replace(mapset);

                        (user, best)
                    }
                    (Err(_), Ok(user), Ok(best)) => {
                        let mapset = match ctx.osu().beatmapset(mapset_id).await {
                            Ok(mapset) => mapset,
                            Err(why) => {
                                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                                return Err(why.into());
                            }
                        };

                        map.mapset.replace(mapset);

                        (user, best)
                    }
                };

                (user, map, Some((score, best)))
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
        // Either the user, map, or user score on the map don't exist
        Err(OsuError::NotFound) => {
            let map = match ctx.psql().get_beatmap(map_id, true).await {
                Ok(map) => map,
                Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
                    Ok(map) => {
                        if let Err(why) = ctx.psql().insert_beatmap(&map).await {
                            unwind_error!(warn, why, "Error while inserting compare map: {}");
                        }

                        map
                    }
                    Err(OsuError::NotFound) => {
                        let content = format!("There is no map with id {}", map_id);

                        return msg.error(&ctx, content).await;
                    }
                    Err(why) => {
                        let _ = msg.send_response(&ctx, OSU_API_ISSUE).await;

                        return Err(why.into());
                    }
                },
            };

            let user = match request_user(&ctx, name.as_str(), Some(map.mode)).await {
                Ok(user) => user,
                Err(OsuError::NotFound) => {
                    let content = format!("Could not find user `{}`", name);

                    return msg.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            };

            (user, map, None)
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    if map.mode == GameMode::MNA {
        let content = "Can't fix mania scores \\:(";

        return msg.error(&ctx, content).await;
    }

    let unchoked_pp = match scores {
        Some((ref mut score, _)) => {
            if score.pp.is_some() && !needs_unchoking(&score, &map) {
                None
            } else {
                match unchoke_pp(score, &map).await {
                    Ok(pp) => pp,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                }
            }
        }
        None => None,
    };

    // Check if the next 50 top scores are required
    if let Some((_, best)) = scores.as_mut().filter(|_| {
        unchoked_pp.is_some() || matches!(map.status, RankStatus::Ranked | RankStatus::Approved)
    }) {
        let best_fut = ctx
            .osu()
            .user_scores(user.user_id)
            .offset(50)
            .limit(50)
            .best()
            .mode(map.mode);

        match best_fut.await {
            Ok(mut scores) => best.append(&mut scores),
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }

        process_tracking(&ctx, map.mode, best, Some(&user)).await;
    }

    let gb = ctx.map_garbage_collector(&map);

    let embed = FixScoreEmbed::new(user, map, scores, unchoked_pp, arg_mods)
        .into_builder()
        .build();

    msg.build_response(&ctx, |m| m.embed(embed)).await?;

    // Set map on garbage collection list if unranked
    gb.execute(&ctx).await;

    Ok(())
}

/// Returns (actual pp, unchoked pp) tuple
async fn unchoke_pp(score: &mut Score, map: &Beatmap) -> BotResult<Option<f32>> {
    let map_path = prepare_beatmap_file(map.map_id).await?;
    let file = File::open(map_path).await.map_err(PPError::from)?;
    let rosu_map = Map::parse(file).await.map_err(PPError::from)?;
    let mods = score.mods.bits();

    let attributes = if score.pp.is_some() {
        None
    } else {
        let pp_result = match map.mode {
            GameMode::STD => OsuPP::new(&rosu_map)
                .mods(mods)
                .combo(score.max_combo as usize)
                .n300(score.statistics.count_300 as usize)
                .n100(score.statistics.count_100 as usize)
                .n50(score.statistics.count_50 as usize)
                .misses(score.statistics.count_miss as usize)
                .calculate(),
            GameMode::MNA => ManiaPP::new(&rosu_map)
                .mods(mods)
                .score(score.score)
                .calculate(),
            GameMode::CTB => FruitsPP::new(&rosu_map)
                .mods(mods)
                .combo(score.max_combo as usize)
                .fruits(score.statistics.count_300 as usize)
                .droplets(score.statistics.count_100 as usize)
                .misses(score.statistics.count_miss as usize)
                .accuracy(score.accuracy)
                .calculate(),
            GameMode::TKO => TaikoPP::new(&rosu_map)
                .combo(score.max_combo as usize)
                .mods(mods)
                .misses(score.statistics.count_miss as usize)
                .accuracy(score.accuracy)
                .calculate(),
        };

        score.pp.replace(pp_result.pp);

        if !needs_unchoking(score, map) {
            return Ok(None);
        }

        Some(pp_result.attributes)
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
            let attributes = attributes.unwrap_or_else(|| stars(&rosu_map, mods, None));

            let attributes = if let StarResult::Fruits(attributes) = attributes {
                attributes
            } else {
                panic!("no ctb attributes after calculating stars for ctb map");
            };

            let total_objects = attributes.max_combo;
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

            calculator.mods(mods).accuracy(acc).calculate().pp
        }
        GameMode::MNA => panic!("can not unchoke mania scores"),
    };

    Ok(Some(unchoked_pp))
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

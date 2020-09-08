use super::{MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32};
use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, ProfileCompareEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use itertools::Itertools;
use rosu::{
    backend::BeatmapRequest,
    models::{Beatmap, GameMode, GameMods, Score},
};
use std::{collections::HashMap, sync::Arc};
use twilight::model::channel::Message;

async fn compare_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    // Parse arguments
    let mut args = MultNameArgs::new(&ctx, args, 2);
    let names = match args.names.len() {
        0 => {
            let content = "You need to specify at least one osu username. \
                If you're not linked, you must specify two names.";
            return msg.error(&ctx, content).await;
        }
        1 => match ctx.get_link(msg.author.id.0) {
            Some(name) => {
                args.names.push(name);
                args.names
            }
            None => {
                let prefix = ctx.config_first_prefix(msg.guild_id);
                let content = format!(
                    "Since you're not linked via `{}link`, \
                    you must specify two names.",
                    prefix
                );
                return msg.error(&ctx, content).await;
            }
        },
        2 => args.names,
        _ => unreachable!(),
    };

    let mut names = names.into_iter();
    let name1 = names.next().unwrap();
    let name2 = names.next().unwrap();
    if name1 == name2 {
        let content = "Give two different names";
        return msg.error(&ctx, content).await;
    }

    // Retrieve all users
    let user_fut1 = ctx.osu_user(&name1, mode);
    let user_fut2 = ctx.osu_user(&name2, mode);
    let (user1, user2) = match tokio::try_join!(user_fut1, user_fut2) {
        Ok((Some(user1), Some(user2))) => (user1, user2),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name1);
            return msg.error(&ctx, content).await;
        }
        Ok((_, None)) => {
            let content = format!("User `{}` was not found", name2);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    if user1.username == user2.username {
        let content = "Give at least two different users";
        return msg.error(&ctx, content).await;
    }

    // Retrieve each user's top scores
    let score_fut1 = user1.get_top_scores(ctx.osu(), 100, mode);
    let score_fut2 = user2.get_top_scores(ctx.osu(), 100, mode);
    let (scores1, scores2) = match tokio::try_join!(score_fut1, score_fut2) {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let content = if scores1.is_empty() {
        Some(format!("No scores data for user `{}`", name1))
    } else if scores2.is_empty() {
        Some(format!("No scores data for user `{}`", name2))
    } else {
        None
    };
    if let Some(content) = content {
        return msg.error(&ctx, content).await;
    }

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores1
        .iter()
        .flat_map(|score| score.beatmap_id)
        .chain(scores2.iter().flat_map(|score| score.beatmap_id))
        .unique()
        .collect();
    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            warn!("Error while getting maps from DB: {}", why);
            HashMap::default()
        }
    };
    debug!("Found {}/{} beatmaps in DB", maps.len(), map_ids.len());
    let retrieving_msg = if map_ids.len() - maps.len() > 10 {
        let content = format!(
            "Retrieving {} maps from the api...",
            map_ids.len() - maps.len()
        );
        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .await
            .ok()
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let mut missing_maps = Vec::new();
    for map_id in map_ids.into_iter() {
        if !maps.contains_key(&map_id) {
            let map_fut = BeatmapRequest::new()
                .map_id(map_id)
                .mode(mode)
                .limit(1)
                .queue_single(ctx.osu());
            match map_fut.await {
                Ok(Some(map)) => {
                    missing_maps.push(map.clone());
                    maps.insert(map.beatmap_id, map);
                }
                Ok(None) => {
                    let content = format!("Beatmap with id `{}` was not found", map_id);
                    return msg.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                    return Err(why.into());
                }
            }
        }
    }
    let profile_result1 = CompareResult::calc(mode, &scores1, &maps);
    let profile_result2 = CompareResult::calc(mode, &scores2, &maps);

    // Accumulate all necessary data
    let data = ProfileCompareEmbed::new(user1, user2, profile_result1, profile_result2);

    // TODO: Combine thumbnails

    // Creating the embed
    let embed = data.build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[short_desc("Compare profile stats between two players")]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("oc", "compareosu", "co")]
pub async fn osucompare(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Compare profile stats between two mania players")]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("mc", "comparemania", "cm")]
pub async fn maniacompare(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Compare profile stats between two taiko players")]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("tc", "comparetaiko", "ct")]
pub async fn taikocompare(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Compare profile stats between two ctb players")]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("cc", "comparectb")]
pub async fn ctbcompare(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::CTB, ctx, msg, args).await
}
pub struct CompareResult {
    pub mode: GameMode,
    pub pp: MinMaxAvgF32,
    pub max_combo: u32,
    pub map_len: MinMaxAvgU32,
}

impl CompareResult {
    fn calc(mode: GameMode, scores: &[Score], maps: &HashMap<u32, Beatmap>) -> Self {
        let mut pp = MinMaxAvgF32::new();
        let mut max_combo = 0;
        let mut map_len = MinMaxAvgF32::new();
        for score in scores.iter() {
            if let Some(score_pp) = score.pp {
                pp.add(score_pp);
            }
            max_combo = max_combo.max(score.max_combo);
            let map = maps.get(&score.beatmap_id.unwrap()).unwrap();
            let seconds_drain = if score.enabled_mods.contains(GameMods::DoubleTime) {
                map.seconds_drain as f32 / 1.5
            } else if score.enabled_mods.contains(GameMods::HalfTime) {
                map.seconds_drain as f32 * 1.5
            } else {
                map.seconds_drain as f32
            };
            map_len.add(seconds_drain);
        }
        Self {
            mode,
            pp,
            max_combo,
            map_len: map_len.into(),
        }
    }
}

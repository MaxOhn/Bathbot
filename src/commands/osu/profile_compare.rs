use super::{MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32};
use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, ProfileCompareEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context, Error,
};

use futures::future::TryFutureExt;
use image::{
    imageops::{overlay, FilterType},
    DynamicImage, ImageBuffer,
    ImageOutputFormat::Png,
    Rgba,
};
use itertools::Itertools;
use rosu::model::{Beatmap, GameMode, GameMods, Score};
use std::{collections::HashMap, sync::Arc};
use twilight_model::channel::Message;

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
    let user_fut1 = ctx.osu().user(name1.as_str()).mode(mode);
    let user_fut2 = ctx.osu().user(name2.as_str()).mode(mode);
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
    let fut = tokio::try_join!(
        user1
            .get_top_scores(ctx.osu())
            .limit(100)
            .mode(mode)
            .map_err(Error::Osu),
        user2
            .get_top_scores(ctx.osu())
            .limit(100)
            .mode(mode)
            .map_err(Error::Osu),
        ctx.clients
            .custom
            .get_osu_profile(user1.user_id, mode, false)
            .map_err(Error::CustomClient),
        ctx.clients
            .custom
            .get_osu_profile(user2.user_id, mode, false)
            .map_err(Error::CustomClient)
    );
    let (scores1, scores2, profile1, profile2) = match fut {
        Ok((scores1, scores2, (profile1, _), (profile2, _))) => {
            (scores1, scores2, profile1, profile2)
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            return Err(why);
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
            match ctx.osu().beatmap().map_id(map_id).await {
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

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &scores1, Some(&user1), &mut maps).await;
    process_tracking(&ctx, mode, &scores2, Some(&user2), &mut maps).await;
    debug!(
        "Processed tracking for profile compare ({},{})",
        user1.username, user2.username
    );

    let profile_result1 = CompareResult::calc(mode, &scores1, &maps);
    let profile_result2 = CompareResult::calc(mode, &scores2, &maps);

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Create the thumbnail
    let thumbnail = match get_combined_thumbnail(&ctx, user1.user_id, user2.user_id).await {
        Ok(thumbnail) => Some(thumbnail),
        Err(why) => {
            warn!("Error while combining avatars: {}", why);
            None
        }
    };

    // Accumulate all necessary data
    let data = ProfileCompareEmbed::new(
        mode,
        user1,
        user2,
        profile_result1,
        profile_result2,
        profile1,
        profile2,
    );

    // Creating the embed
    let embed = data.build().build()?;
    msg.build_response(&ctx, |m| match thumbnail {
        Some(bytes) => m.attachment("avatar_fuse.png", bytes).embed(embed),
        None => m.embed(embed),
    })
    .await?;
    Ok(())
}

#[command]
#[short_desc("Compare profile stats between two players")]
#[long_desc(
    "Compare profile stats between two players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("oc", "compareosu", "co")]
pub async fn osucompare(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Compare profile stats between two mania players")]
#[long_desc(
    "Compare profile stats between two mania players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("ocm")]
pub async fn osucomparemania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Compare profile stats between two taiko players")]
#[long_desc(
    "Compare profile stats between two taiko players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("oct")]
pub async fn osucomparetaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Compare profile stats between two ctb players")]
#[long_desc(
    "Compare profile stats between two ctb players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("occ")]
pub async fn osucomparectb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    compare_main(GameMode::CTB, ctx, msg, args).await
}
pub struct CompareResult {
    pub mode: GameMode,
    pub pp: MinMaxAvgF32,
    pub map_len: MinMaxAvgU32,
}

impl CompareResult {
    fn calc(mode: GameMode, scores: &[Score], maps: &HashMap<u32, Beatmap>) -> Self {
        let mut pp = MinMaxAvgF32::new();
        let mut map_len = MinMaxAvgF32::new();
        for score in scores.iter() {
            if let Some(score_pp) = score.pp {
                pp.add(score_pp);
            }
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
            map_len: map_len.into(),
        }
    }
}

async fn get_combined_thumbnail(ctx: &Context, user_id1: u32, user_id2: u32) -> BotResult<Vec<u8>> {
    let mut img = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(720, 128, Rgba([0, 0, 0, 0])));
    let (pfp1, pfp2) = tokio::try_join!(
        ctx.clients.custom.get_avatar(user_id1),
        ctx.clients.custom.get_avatar(user_id2),
    )?;
    let pfp1 = image::load_from_memory(&pfp1)?.resize_exact(128, 128, FilterType::Lanczos3);
    let pfp2 = image::load_from_memory(&pfp2)?.resize_exact(128, 128, FilterType::Lanczos3);
    overlay(&mut img, &pfp1, 10, 0);
    overlay(&mut img, &pfp2, 582, 0);
    let mut png_bytes: Vec<u8> = Vec::with_capacity(92_160); // 720x128
    img.write_to(&mut png_bytes, Png)?;
    Ok(png_bytes)
}

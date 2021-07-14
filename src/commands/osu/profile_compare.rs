use super::{request_user, MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32};
use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, ProfileCompareEmbed},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, osu::BonusPP, MessageExt},
    BotResult, Context,
};

use image::{
    imageops::{overlay, FilterType},
    DynamicImage, ImageBuffer,
    ImageOutputFormat::Png,
    Rgba,
};
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score, UserStatistics};
use std::sync::Arc;
use twilight_model::channel::Message;

async fn compare_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    // Parse arguments
    let args = MultNameArgs::new(&ctx, args, 2);
    let mut names = args.names.into_iter();

    let (name1, name2) = match (names.next(), names.next()) {
        (Some(name1), Some(name2)) => (name1, name2),
        (Some(name1), None) => match ctx.get_link(msg.author.id.0) {
            Some(name2) => (name1, name2),
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
        (None, _) => {
            let content = "You need to specify at least one osu username. \
                If you're not linked, you must specify two names.";

            return msg.error(&ctx, content).await;
        }
    };
    if name1 == name2 {
        let content = "Give two different names";

        return msg.error(&ctx, content).await;
    }

    // Retrieve all users and their scores
    let user_fut1 = request_user(&ctx, &name1, Some(mode));
    let user_fut2 = request_user(&ctx, &name2, Some(mode));
    let scores_fut_u1_1 = ctx
        .osu()
        .user_scores(name1.as_str())
        .mode(mode)
        .best()
        .limit(50);
    let scores_fut_u2_1 = ctx
        .osu()
        .user_scores(name2.as_str())
        .mode(mode)
        .best()
        .limit(50);

    let scores_fut_u1_2 = ctx
        .osu()
        .user_scores(name1.as_str())
        .best()
        .mode(mode)
        .offset(50)
        .limit(50);

    let scores_fut_u2_2 = ctx
        .osu()
        .user_scores(name2.as_str())
        .best()
        .mode(mode)
        .offset(50)
        .limit(50);

    let fut_result = tokio::try_join!(
        user_fut1,
        user_fut2,
        scores_fut_u1_1,
        scores_fut_u1_2,
        scores_fut_u2_1,
        scores_fut_u2_2,
    );

    let (user1, user2, mut scores1, mut scores2) = match fut_result {
        Ok((user1, user2, mut scores1, mut scores1_, mut scores2, mut scores2_)) => {
            scores1.append(&mut scores1_);
            scores2.append(&mut scores2_);

            (user1, user2, scores1, scores2)
        }
        Err(OsuError::NotFound) => {
            let content = "At least one of the players was not found";

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    if user1.user_id == user2.user_id {
        let content = "Give at least two different users";

        return msg.error(&ctx, content).await;
    }

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

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores1, Some(&user1)).await;
    process_tracking(&ctx, mode, &mut scores2, Some(&user2)).await;

    debug!(
        "Processed tracking for profile compare ({},{})",
        user1.username, user2.username
    );

    let profile_result1 = CompareResult::calc(mode, &scores1, user1.statistics.as_ref().unwrap());
    let profile_result2 = CompareResult::calc(mode, &scores2, user2.statistics.as_ref().unwrap());

    // Create the thumbnail
    let thumbnail =
        match get_combined_thumbnail(&ctx, user1.avatar_url.as_str(), user2.avatar_url.as_str())
            .await
        {
            Ok(thumbnail) => Some(thumbnail),
            Err(why) => {
                unwind_error!(warn, why, "Error while combining avatars: {}");

                None
            }
        };

    // Accumulate all necessary data
    let data = ProfileCompareEmbed::new(mode, user1, user2, profile_result1, profile_result2);

    // Creating the embed
    let embed = data.into_builder().build();

    msg.build_response(&ctx, |m| match thumbnail {
        Some(bytes) => m.file("avatar_fuse.png", bytes).embed(embed),
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
    pub bonus_pp: f32,
}

impl CompareResult {
    fn calc(mode: GameMode, scores: &[Score], stats: &UserStatistics) -> Self {
        let mut pp = MinMaxAvgF32::new();
        let mut map_len = MinMaxAvgF32::new();
        let mut bonus_pp = BonusPP::new();

        for (i, score) in scores.iter().enumerate() {
            if let Some(score_pp) = score.pp {
                pp.add(score_pp);
            }

            if let Some(weighted_pp) = score.weight.map(|w| w.pp) {
                bonus_pp.update(weighted_pp, i);
            }

            let map = score.map.as_ref().unwrap();

            let seconds_drain = if score.mods.contains(GameMods::DoubleTime) {
                map.seconds_drain as f32 / 1.5
            } else if score.mods.contains(GameMods::HalfTime) {
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
            bonus_pp: bonus_pp.calculate(stats),
        }
    }
}

async fn get_combined_thumbnail(
    ctx: &Context,
    user1_url: &str,
    user2_url: &str,
) -> BotResult<Vec<u8>> {
    let mut img = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(720, 128, Rgba([0, 0, 0, 0])));

    let (pfp1, pfp2) = tokio::try_join!(
        ctx.clients.custom.get_avatar_with_url(user1_url),
        ctx.clients.custom.get_avatar_with_url(user2_url),
    )?;

    let pfp1 = image::load_from_memory(&pfp1)?.resize_exact(128, 128, FilterType::Lanczos3);
    let pfp2 = image::load_from_memory(&pfp2)?.resize_exact(128, 128, FilterType::Lanczos3);
    overlay(&mut img, &pfp1, 10, 0);
    overlay(&mut img, &pfp2, 582, 0);
    let mut png_bytes: Vec<u8> = Vec::with_capacity(92_160); // 720x128
    img.write_to(&mut png_bytes, Png)?;

    Ok(png_bytes)
}

use std::{borrow::Cow, io::Cursor, sync::Arc};

use bathbot_macros::{command, SlashCommand};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    numbers::MinMaxAvg,
    osu::{BonusPP, UserStats},
    MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use image::{
    imageops::{overlay, FilterType},
    DynamicImage, ImageBuffer,
    ImageOutputFormat::Png,
    Rgba,
};
use rosu_v2::{
    prelude::{GameMode, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

use super::{CompareProfile, AT_LEAST_ONE};
use crate::{
    commands::{osu::UserExtraction, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, ProfileCompareEmbed},
    manager::redis::osu::UserArgs,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, Default, SlashCommand)]
#[command(
    name = "cp",
    desc = "Compare two profiles",
    help = "Compare profile stats between two players.\n\
    Note:\n\
    - PC peak = Monthly playcount peak\n\
    - PP spread = PP difference between the top score and the 100th score"
)]
#[allow(unused)]
pub struct Cp<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name1: Option<Cow<'a, str>>,
    #[command(desc = "Specify a username")]
    name2: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord1: Option<Id<UserMarker>>,
    #[command(desc = "Specify a linked discord user")]
    discord2: Option<Id<UserMarker>>,
}

async fn slash_cp(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = CompareProfile::from_interaction(command.input_data())?;

    profile(ctx, (&mut command).into(), args).await
}

async fn extract_user_id(ctx: &Context, args: &mut CompareProfile<'_>) -> UserExtraction {
    if let Some(name) = args.name1.take().or_else(|| args.name2.take()) {
        UserExtraction::Id(UserId::Name(name.as_ref().into()))
    } else if let Some(discord) = args.discord1.take().or_else(|| args.discord2.take()) {
        match ctx.user_config().osu_id(discord).await {
            Ok(Some(user_id)) => UserExtraction::Id(UserId::Id(user_id)),
            Ok(None) => {
                UserExtraction::Content(format!("<@{discord}> is not linked to an osu!profile"))
            }
            Err(err) => UserExtraction::Err(err),
        }
    } else {
        UserExtraction::None
    }
}

pub(super) async fn profile(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mut args: CompareProfile<'_>,
) -> Result<()> {
    let user_id1 = match extract_user_id(&ctx, &mut args).await {
        UserExtraction::Id(user_id) => user_id,
        UserExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        UserExtraction::Content(content) => return orig.error(&ctx, content).await,
        UserExtraction::None => return orig.error(&ctx, AT_LEAST_ONE).await,
    };

    let user_id2 = match extract_user_id(&ctx, &mut args).await {
        UserExtraction::Id(user_id) => user_id,
        UserExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        UserExtraction::Content(content) => return orig.error(&ctx, content).await,
        UserExtraction::None => match ctx.user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => {
                let content =
                    "Since you're not linked with the `/link` command, you must specify two names.";

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    if user_id1 == user_id2 {
        return orig.error(&ctx, "Give two different names").await;
    }

    let mode = match args.mode {
        Some(mode) => mode.into(),
        None => match ctx.user_config().mode(orig.user_id()?).await {
            Ok(mode) => mode.unwrap_or(GameMode::Osu),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    // Retrieve all users and their scores
    let user_args1 = UserArgs::rosu_id(&ctx, &user_id1).await.mode(mode);
    let user_args2 = UserArgs::rosu_id(&ctx, &user_id2).await.mode(mode);
    let score_args = ctx.osu_scores().top().limit(100);

    let fut1 = score_args.exec_with_user(user_args1);
    let fut2 = score_args.exec_with_user(user_args2);

    let (user1, user2, scores1, scores2) = match tokio::try_join!(fut1, fut2) {
        Ok(((user1, scores1), (user2, scores2))) => (user1, user2, scores1, scores2),
        Err(OsuError::NotFound) => {
            let content = "At least one of the players was not found";

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user and scores");

            return Err(err);
        }
    };

    if user1.user_id() == user2.user_id() {
        let content = "Give two different users";

        return orig.error(&ctx, content).await;
    }

    let content = if scores1.is_empty() {
        Some(format!("No scores data for user `{}`", user1.username()))
    } else if scores2.is_empty() {
        Some(format!("No scores data for user `{}`", user2.username()))
    } else {
        None
    };

    if let Some(content) = content {
        return orig.error(&ctx, content).await;
    }

    let thumbnail_fut = get_combined_thumbnail(&ctx, user1.avatar_url(), user2.avatar_url());

    let score_ranks_fut = ctx
        .client()
        .get_respektive_users([user1.user_id(), user2.user_id()], mode);

    let (thumbnail_res, score_ranks_res) = tokio::join!(thumbnail_fut, score_ranks_fut);

    // Create the thumbnail
    let thumbnail = match thumbnail_res {
        Ok(thumbnail) => Some(thumbnail),
        Err(err) => {
            warn!(?err, "Failed to combine avatars");

            None
        }
    };

    let (score_rank1, score_rank2) = match score_ranks_res {
        Ok(mut iter) => {
            let rank1 = iter.next().flatten().map(|user| user.rank);
            let rank2 = iter.next().flatten().map(|user| user.rank);

            (rank1, rank2)
        }
        Err(err) => {
            warn!(?err, "Failed to get respektive users");

            (None, None)
        }
    };

    let profile_result1 = CompareResult::calc(mode, &scores1, user1.stats(), score_rank1);
    let profile_result2 = CompareResult::calc(mode, &scores2, user2.stats(), score_rank2);

    // Creating the embed
    let embed_data =
        ProfileCompareEmbed::new(mode, &user1, &user2, profile_result1, profile_result2);
    let embed = embed_data.build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = thumbnail {
        builder = builder.attachment("avatar_fuse.png", bytes);
    }

    orig.create_message(&ctx, builder).await?;

    Ok(())
}

#[command]
#[desc("Compare profile stats between two players")]
#[help(
    "Compare profile stats between two players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("pc", "profilecompareosu", "pco", "compareprofile")]
#[group(Osu)]
async fn prefix_profilecompare(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = CompareProfile::args(None, args);

    profile(ctx, CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Compare profile stats between two mania players")]
#[help(
    "Compare profile stats between two mania players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("pcm", "compareprofilemania")]
#[group(Mania)]
async fn prefix_profilecomparemania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = CompareProfile::args(Some(GameModeOption::Mania), args);

    profile(ctx, CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Compare profile stats between two taiko players")]
#[help(
    "Compare profile stats between two taiko players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("pct", "compareprofiletaiko")]
#[group(Taiko)]
async fn prefix_profilecomparetaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = CompareProfile::args(Some(GameModeOption::Taiko), args);

    profile(ctx, CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Compare profile stats between two ctb players")]
#[help(
    "Compare profile stats between two ctb players.\n\
    Note:\n \
    - PC peak = Monthly playcount peak\n \
    - PP spread = PP difference between top score and 100th score"
)]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases(
    "pcc",
    "profilecomparecatch",
    "compareprofilectb",
    "compareprofilecatch"
)]
#[group(Catch)]
async fn prefix_profilecomparectb(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = CompareProfile::args(Some(GameModeOption::Catch), args);

    profile(ctx, CommandOrigin::from_msg(msg, permissions), args).await
}
pub struct CompareResult {
    pub mode: GameMode,
    pub pp: MinMaxAvg<f32>,
    pub map_len: MinMaxAvg<u32>,
    pub bonus_pp: f32,
    pub top1pp: f32,
    pub score_rank: Option<u32>,
    pub hits: u32,
    pub misses: u32,
}

impl CompareResult {
    fn calc(
        mode: GameMode,
        scores: &[Score],
        stats: impl UserStats,
        score_rank: Option<u32>,
    ) -> Self {
        let mut pp = MinMaxAvg::new();
        let mut map_len = MinMaxAvg::new();
        let mut bonus_pp = BonusPP::new();

        let mut misses = 0;
        let mut hits = 0;

        for (i, score) in scores.iter().enumerate() {
            if let Some(score_pp) = score.pp {
                pp.add(score_pp);
            }

            if let Some(weighted_pp) = score.weight.map(|w| w.pp) {
                bonus_pp.update(weighted_pp, i);
            }

            let map = score.map.as_ref().unwrap();

            let seconds_drain = if let Some(clock_rate) = score.mods.clock_rate() {
                map.seconds_drain as f32 / clock_rate
            } else {
                map.seconds_drain as f32
            };

            map_len.add(seconds_drain);

            hits += score.total_hits() - score.statistics.count_miss;
            misses += score.statistics.count_miss;
        }

        Self {
            mode,
            pp,
            map_len: map_len.into(),
            bonus_pp: bonus_pp.calculate(stats),
            top1pp: scores.first().and_then(|score| score.pp).unwrap_or(0.0),
            score_rank,
            hits,
            misses,
        }
    }
}

async fn get_combined_thumbnail(
    ctx: &Context,
    user1_url: &str,
    user2_url: &str,
) -> Result<Vec<u8>> {
    let mut img = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(720, 128, Rgba([0, 0, 0, 0])));

    let (pfp1, pfp2) = tokio::try_join!(
        ctx.client().get_avatar(user1_url),
        ctx.client().get_avatar(user2_url),
    )
    .wrap_err("Failed to get avatar")?;

    let pfp1 = image::load_from_memory(&pfp1)
        .wrap_err("Failed to load pfp1 from memory")?
        .resize_exact(128, 128, FilterType::Lanczos3);

    let pfp2 = image::load_from_memory(&pfp2)
        .wrap_err("Failed to load pfp2 from memory")?
        .resize_exact(128, 128, FilterType::Lanczos3);

    overlay(&mut img, &pfp1, 10, 0);
    overlay(&mut img, &pfp2, 582, 0);
    let png_bytes: Vec<u8> = Vec::with_capacity(92_160); // 720x128

    let mut cursor = Cursor::new(png_bytes);
    img.write_to(&mut cursor, Png)
        .wrap_err("Failed to encode image")?;

    Ok(cursor.into_inner())
}

impl<'m> CompareProfile<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Self {
        let mut name1 = None;
        let mut name2 = None;
        let mut discord1 = None;
        let mut discord2 = None;

        for arg in args.take(2) {
            if let Some(id) = matcher::get_mention_user(arg) {
                if discord1.is_none() {
                    discord1 = Some(id);
                } else {
                    discord2 = Some(id);
                }
            } else if name1.is_none() {
                name1 = Some(arg.into());
            } else {
                name2 = Some(arg.into());
            }
        }

        Self {
            mode,
            name1,
            name2,
            discord1,
            discord2,
        }
    }
}

use super::{MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32, AT_LEAST_ONE};
use crate::{
    embeds::{EmbedData, ProfileCompareEmbed},
    tracking::process_tracking,
    util::{
        constants::{common_literals::MODE, GENERAL_ISSUE, OSU_API_ISSUE},
        matcher,
        osu::BonusPP,
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use image::{
    imageops::{overlay, FilterType},
    DynamicImage, ImageBuffer,
    ImageOutputFormat::Png,
    Rgba,
};
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score, UserStatistics};
use std::{borrow::Cow, sync::Arc};
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _profilecompare(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: ProfileArgs,
) -> BotResult<()> {
    let ProfileArgs { name1, name2, mode } = args;

    let name1 = match name1 {
        Some(name) => name,
        None => {
            let content =
                "Since you're not linked with the `link` command, you must specify two names.";

            return data.error(&ctx, content).await;
        }
    };

    if name1 == name2 {
        return data.error(&ctx, "Give two different names").await;
    }

    // Retrieve all users and their scores
    let user_fut1 = super::request_user(&ctx, &name1, Some(mode));
    let user_fut2 = super::request_user(&ctx, &name2, Some(mode));

    let scores_fut_u1 = ctx
        .osu()
        .user_scores(name1.as_str())
        .mode(mode)
        .best()
        .limit(100);

    let scores_fut_u2 = ctx
        .osu()
        .user_scores(name2.as_str())
        .mode(mode)
        .best()
        .limit(100);

    let fut_result = tokio::try_join!(user_fut1, user_fut2, scores_fut_u1, scores_fut_u2,);

    let (user1, user2, mut scores1, mut scores2) = match fut_result {
        Ok((user1, user2, scores1, scores2)) => (user1, user2, scores1, scores2),
        Err(OsuError::NotFound) => {
            let content = "At least one of the players was not found";

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    if user1.user_id == user2.user_id {
        let content = "Give two different users";

        return data.error(&ctx, content).await;
    }

    let content = if scores1.is_empty() {
        Some(format!("No scores data for user `{}`", name1))
    } else if scores2.is_empty() {
        Some(format!("No scores data for user `{}`", name2))
    } else {
        None
    };

    if let Some(content) = content {
        return data.error(&ctx, content).await;
    }

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores1, Some(&user1)).await;
    process_tracking(&ctx, mode, &mut scores2, Some(&user2)).await;

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

    // Creating the embed
    let embed_data = ProfileCompareEmbed::new(mode, user1, user2, profile_result1, profile_result2);
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = thumbnail.as_deref() {
        builder = builder.file("avatar_fuse.png", bytes);
    }

    data.create_message(&ctx, builder).await?;

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
pub async fn osucompare(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, msg.author.id, GameMode::STD).await {
                Ok(Ok(profile_args)) => {
                    _profilecompare(ctx, CommandData::Message { msg, args, num }, profile_args)
                        .await
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
pub async fn osucomparemania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, msg.author.id, GameMode::MNA).await {
                Ok(Ok(profile_args)) => {
                    _profilecompare(ctx, CommandData::Message { msg, args, num }, profile_args)
                        .await
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
pub async fn osucomparetaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, msg.author.id, GameMode::TKO).await {
                Ok(Ok(profile_args)) => {
                    _profilecompare(ctx, CommandData::Message { msg, args, num }, profile_args)
                        .await
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
pub async fn osucomparectb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, msg.author.id, GameMode::CTB).await {
                Ok(Ok(profile_args)) => {
                    _profilecompare(ctx, CommandData::Message { msg, args, num }, profile_args)
                        .await
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
        ctx.clients.custom.get_avatar(user1_url),
        ctx.clients.custom.get_avatar(user2_url),
    )?;

    let pfp1 = image::load_from_memory(&pfp1)?.resize_exact(128, 128, FilterType::Lanczos3);
    let pfp2 = image::load_from_memory(&pfp2)?.resize_exact(128, 128, FilterType::Lanczos3);
    overlay(&mut img, &pfp1, 10, 0);
    overlay(&mut img, &pfp2, 582, 0);
    let mut png_bytes: Vec<u8> = Vec::with_capacity(92_160); // 720x128
    img.write_to(&mut png_bytes, Png)?;

    Ok(png_bytes)
}

pub(super) struct ProfileArgs {
    name1: Option<Name>,
    name2: Name,
    mode: GameMode,
}

const COMPARE_PROFILE: &str = "compare profile";

impl ProfileArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        mut mode: GameMode,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let config = ctx.user_config(author_id).await?;

        if mode == GameMode::STD {
            mode = config.mode.unwrap_or(GameMode::STD);
        }

        let name2 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                    Some(name) => name,
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Ok(Err(content.into()));
                    }
                },
                None => arg.into(),
            },
            None => return Ok(Err(AT_LEAST_ONE.into())),
        };

        let args = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                    Some(name) => Self {
                        name1: Some(name2),
                        name2: name,
                        mode,
                    },
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Ok(Err(content.into()));
                    }
                },
                None => Self {
                    name1: Some(name2),
                    name2: arg.into(),
                    mode,
                },
            },
            None => Self {
                name1: config.osu_username,
                name2,
                mode,
            },
        };

        Ok(Ok(args))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut name1 = None;
        let mut name2 = None;
        let mut mode = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    MODE => mode = parse_mode_option!(value, "compare profile"),
                    "name1" => name1 = Some(value.into()),
                    "name2" => name2 = Some(value.into()),
                    "discord1" => match value.parse() {
                        Ok(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                            Some(name) => name1 = Some(name),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => bail_cmd_option!("compare profile discord1", string, value),
                    },
                    "discord2" => match value.parse() {
                        Ok(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                            Some(name) => name2 = Some(name),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => bail_cmd_option!("compare profile discord2", string, value),
                    },
                    _ => bail_cmd_option!(COMPARE_PROFILE, string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(COMPARE_PROFILE, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(COMPARE_PROFILE, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(COMPARE_PROFILE, subcommand, name)
                }
            }
        }

        let (name1, name2) = match (name1, name2) {
            (name1, Some(name)) => (name1, name),
            (Some(name), None) => (None, name),
            (None, None) => return Ok(Err(AT_LEAST_ONE.into())),
        };

        let name1 = match name1 {
            Some(name) => Some(name),
            None => ctx.user_config(author_id).await?.osu_username,
        };

        let mode = mode.unwrap_or(GameMode::STD);

        Ok(Ok(ProfileArgs { name1, name2, mode }))
    }
}

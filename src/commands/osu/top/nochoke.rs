use super::ErrorType;
use crate::{
    bail,
    database::UserConfig,
    embeds::{EmbedData, NoChokeEmbed},
    error::PPError,
    pagination::{NoChokePagination, Pagination},
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        numbers,
        osu::prepare_beatmap_file,
        MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use rosu_pp::{Beatmap as Map, FruitsPP, OsuPP, StarResult, TaikoPP};
use rosu_v2::prelude::{GameMode, OsuError};
use std::{borrow::Cow, cmp::Ordering, sync::Arc};
use tokio::fs::File;
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _nochokes(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: NochokeArgs,
) -> BotResult<()> {
    let NochokeArgs { config, miss_limit } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their top scores
    let user_fut = super::request_user(&ctx, &name, mode).map_err(From::from);
    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let scores_fut = super::prepare_scores(&ctx, scores_fut);

    let (mut user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => (user, scores),
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    // Unchoke scores asynchronously
    let unchoke_fut = scores
        .into_iter()
        .enumerate()
        .map(|(mut i, score)| async move {
            i += 1;
            let map = score.map.as_ref().unwrap();
            let mut unchoked = score.clone();

            let many_misses = miss_limit
                .filter(|&limit| score.statistics.count_miss > limit)
                .is_some();

            // Skip unchoking because it has too many misses or because its a convert
            if many_misses || map.convert {
                return Ok((i, score, unchoked));
            }

            let map_path = prepare_beatmap_file(map.map_id).await?;
            let file = File::open(map_path).await.map_err(PPError::from)?;
            let rosu_map = Map::parse(file).await.map_err(PPError::from)?;
            let mods = score.mods.bits();

            match map.mode {
                GameMode::STD
                    if score.statistics.count_miss > 0
                        || score.max_combo < map.max_combo.unwrap_or(5).saturating_sub(5) =>
                {
                    let total_objects = map.count_objects() as usize;

                    let mut count300 = score.statistics.count_300 as usize;

                    let count_hits = total_objects - score.statistics.count_miss as usize;
                    let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                    let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                    count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                    let count100 = (score.statistics.count_100 + new100s) as usize;
                    let count50 = score.statistics.count_50 as usize;

                    let pp_result = OsuPP::new(&rosu_map)
                        .mods(mods)
                        .n300(count300)
                        .n100(count100)
                        .n50(count50)
                        .calculate();

                    unchoked.statistics.count_300 = count300 as u32;
                    unchoked.statistics.count_100 = count100 as u32;
                    unchoked.max_combo = map.max_combo.unwrap_or(0);
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp);
                    unchoked.grade = unchoked.grade(None);
                    unchoked.accuracy = unchoked.accuracy();
                }
                GameMode::CTB if score.max_combo != map.max_combo.unwrap_or(0) => {
                    let attributes = match rosu_pp::fruits::stars(&rosu_map, mods, None) {
                        StarResult::Fruits(attributes) => attributes,
                        _ => bail!("no ctb attributes after calculating stars for ctb map"),
                    };

                    let total_objects = attributes.max_combo;
                    let passed_objects = (score.statistics.count_300
                        + score.statistics.count_100
                        + score.statistics.count_miss)
                        as usize;

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

                    let pp_result = FruitsPP::new(&rosu_map)
                        .attributes(attributes)
                        .mods(mods)
                        .fruits(n_fruits)
                        .droplets(n_droplets)
                        .tiny_droplets(n_tiny_droplets)
                        .tiny_droplet_misses(n_tiny_droplet_misses)
                        .calculate();

                    let hits = n_fruits + n_droplets + n_tiny_droplets;
                    let total = hits + n_tiny_droplet_misses;

                    let acc = if total == 0 {
                        0.0
                    } else {
                        100.0 * hits as f32 / total as f32
                    };

                    unchoked.statistics.count_300 = n_fruits as u32;
                    unchoked.statistics.count_katu = n_tiny_droplet_misses as u32;
                    unchoked.statistics.count_100 = n_droplets as u32;
                    unchoked.statistics.count_50 = n_tiny_droplets as u32;
                    unchoked.max_combo = total_objects as u32;
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp);
                    unchoked.grade = unchoked.grade(Some(acc));
                    unchoked.accuracy = unchoked.accuracy();
                }
                GameMode::TKO if score.statistics.count_miss > 0 => {
                    let total_objects = map.count_circles as usize;
                    let passed_objects = score.total_hits() as usize;

                    let mut count300 = score.statistics.count_300 as usize
                        + total_objects.saturating_sub(passed_objects);

                    let count_hits = total_objects - score.statistics.count_miss as usize;
                    let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                    let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                    count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                    let count100 = (score.statistics.count_100 + new100s) as usize;

                    let acc = 100.0 * (2 * count300 + count100) as f32 / (2 * total_objects) as f32;

                    let pp_result = TaikoPP::new(&rosu_map).mods(mods).accuracy(acc).calculate();

                    unchoked.statistics.count_300 = count300 as u32;
                    unchoked.statistics.count_100 = count100 as u32;
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp);
                    unchoked.grade = unchoked.grade(Some(acc));
                    unchoked.accuracy = unchoked.accuracy();
                }
                GameMode::MNA => bail!("can not unchoke mania scores"),
                _ => {} // Nothing to unchoke
            }

            Ok::<_, Error>((i, score, unchoked))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect();

    let mut scores_data: Vec<_> = match unchoke_fut.await {
        Ok(scores_data) => scores_data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Calculate bonus pp
    let actual_pp: f32 = scores_data
        .iter()
        .filter_map(|(_, s, ..)| s.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;

    // Sort by unchoked pp
    scores_data.sort_unstable_by(|(_, _, s1), (_, _, s2)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate total user pp without chokes
    let mut unchoked_pp: f32 = scores_data
        .iter()
        .enumerate()
        .map(|(i, (_, _, s))| s.pp.unwrap_or(0.0) * 0.95_f32.powi(i as i32))
        .sum();

    unchoked_pp = (100.0 * (unchoked_pp + bonus_pp)).round() / 100.0;

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, scores_data.len());
    let embed_data_fut =
        NoChokeEmbed::new(&user, scores_data.iter().take(5), unchoked_pp, (1, pages));
    let embed = embed_data_fut.await.into_builder().build();

    let content = format!(
        "No-choke top {}scores for `{}`:",
        match mode {
            GameMode::STD => "",
            GameMode::TKO => "taiko ",
            GameMode::CTB => "ctb ",
            GameMode::MNA => panic!("can not unchoke mania scores"),
        },
        name
    );

    // Creating the embed
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // Add maps of scores to DB
    let scores_iter = scores_data.iter().map(|(_, score, _)| score);

    if let Err(why) = ctx.psql().store_scores_maps(scores_iter).await {
        unwind_error!(warn, why, "Error while adding score maps to DB: {}")
    }

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = NoChokePagination::new(response, user, scores_data, unchoked_pp);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 90).await {
            unwind_error!(warn, why, "Pagination error (nochokes): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Unchoke a user's top100")]
#[long_desc(
    "Display a user's top plays if no score in their top100 would be a choke.\n
    If a number is specified, I will only unchoke scores with at most that many misses"
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3", "vaxei 5")]
#[aliases("nc", "nochoke")]
async fn nochokes(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match NochokeArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut nochoke_args)) => {
                    nochoke_args.config.mode.get_or_insert(GameMode::STD);

                    _nochokes(ctx, CommandData::Message { msg, args, num }, nochoke_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("Unchoke a user's taiko top100")]
#[long_desc(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3", "vaxei 5")]
#[aliases("nct", "nochoketaiko")]
async fn nochokestaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match NochokeArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut nochoke_args)) => {
                    nochoke_args.config.mode = Some(GameMode::TKO);

                    _nochokes(ctx, CommandData::Message { msg, args, num }, nochoke_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("Unchoke a user's ctb top100")]
#[long_desc(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[example("badewanne3", "vaxei 5")]
#[aliases("ncc", "nochokectb")]
async fn nochokesctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match NochokeArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut nochoke_args)) => {
                    nochoke_args.config.mode = Some(GameMode::CTB);

                    _nochokes(ctx, CommandData::Message { msg, args, num }, nochoke_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

pub(super) struct NochokeArgs {
    config: UserConfig,
    miss_limit: Option<u32>,
}

const TOP_NOCHOKE: &str = "top nochoke";

impl NochokeArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, &'static str>> {
        let mut config = ctx.user_config(author_id).await?;

        if let Some(arg) = args.next() {
            match Args::check_user_mention(ctx, arg).await? {
                Ok(osu) => config.osu = Some(osu),
                Err(content) => return Ok(Err(content)),
            }
        }

        let miss_limit = match args.next().map(str::parse) {
            Some(Ok(num)) => Some(num),
            Some(Err(_)) => {
                let content = "Failed to parse second argument as miss limit.\n\
                    Be sure you specify it as a positive integer.";

                return Ok(Err(content));
            }
            None => None,
        };

        Ok(Ok(Self { config, miss_limit }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut miss_limit = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MODE => config.mode = parse_mode_option!(value, "top nochoke"),
                    DISCORD => config.osu = Some(parse_discord_option!(ctx, value, "top nochoke")),
                    _ => bail_cmd_option!(TOP_NOCHOKE, string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    "miss_limit" => miss_limit = Some(value.max(0) as u32),
                    _ => bail_cmd_option!(TOP_NOCHOKE, integer, name),
                },
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(TOP_NOCHOKE, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(TOP_NOCHOKE, subcommand, name)
                }
            }
        }

        Ok(Ok(Self { config, miss_limit }))
    }
}

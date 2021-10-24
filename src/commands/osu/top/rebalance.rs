use super::ErrorType;
use crate::{
    database::UserConfig,
    embeds::{EmbedData, TopIfEmbed},
    error::PPError,
    pagination::{Pagination, TopIfPagination},
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        numbers,
        osu::prepare_beatmap_file,
        CowUtils, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use rosu_pp_newer::{osu_delta, osu_sotarks, osu_xexxar};
use rosu_v2::prelude::{GameMode, OsuError, Score};
use std::{borrow::Cow, cmp::Ordering, sync::Arc};
use tokio::fs::File;
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) struct RebalanceArgs {
    pub config: UserConfig,
    pub version: RebalanceVersion,
}

const TOP_REBALANCE: &str = "top rebalance";

impl RebalanceArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, &'static str>> {
        let mut config = ctx.user_config(author_id).await?;

        let version = match args.next().map(CowUtils::cow_to_ascii_lowercase).as_deref() {
            Some("xexxar") => RebalanceVersion::Xexxar,
            Some("delta") | Some("delta_t") | Some("deltat") => RebalanceVersion::Delta,
            Some("sotarks") => RebalanceVersion::Sotarks,
            _ => {
                let content = "The first argument must be the version name so either \
                    `xexxar`, `delta`, or `sotarks`.";

                return Ok(Err(content));
            }
        };

        if let Some(arg) = args.next() {
            match Args::check_user_mention(ctx, arg).await? {
                Ok(osu) => config.osu = Some(osu),
                Err(content) => return Ok(Err(content)),
            }
        }

        Ok(Ok(Self { config, version }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut version = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    DISCORD => {
                        config.osu = Some(parse_discord_option!(ctx, value, "top rebalance"))
                    }
                    "version" => match value.as_str() {
                        "delta_t" => version = Some(RebalanceVersion::Delta),
                        "sotarks" => version = Some(RebalanceVersion::Sotarks),
                        "xexxar" => version = Some(RebalanceVersion::Xexxar),
                        _ => {
                            bail_cmd_option!("top rebalance version", string, value)
                        }
                    },
                    _ => bail_cmd_option!(TOP_REBALANCE, string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(TOP_REBALANCE, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(TOP_REBALANCE, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(TOP_REBALANCE, subcommand, name)
                }
            }
        }

        let args = Self {
            version: version.ok_or(Error::InvalidCommandOptions)?,
            config,
        };

        Ok(Ok(args))
    }
}

#[derive(Copy, Clone)]
pub(super) enum RebalanceVersion {
    Delta,
    Sotarks,
    Xexxar,
}

pub(super) async fn _rebalance(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RebalanceArgs,
) -> BotResult<()> {
    let RebalanceArgs { config, version } = args;
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

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|score| score.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;

    let scores_fut = scores
        .into_iter()
        .enumerate()
        .map(|(mut i, mut score)| async move {
            i += 1;
            let map = score.map.as_ref().unwrap();

            if map.convert {
                return Ok((i, score, None));
            }

            // Calculate pp values
            let max_pp = match version {
                RebalanceVersion::Delta => osu_delta(&mut score).await?,
                RebalanceVersion::Sotarks => osu_sotarks(&mut score).await?,
                RebalanceVersion::Xexxar => osu_xexxar(&mut score).await?,
            };

            Ok((i, score, Some(max_pp)))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect::<Vec<_>>();

    let mut scores_data = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sort by adjusted pp
    scores_data.sort_unstable_by(|(_, s1, _), (_, s2, _)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate adjusted pp
    let adjusted_pp: f32 = scores_data
        .iter()
        .enumerate()
        .map(|(i, (_, Score { pp, .. }, ..))| pp.unwrap_or(0.0) * 0.95_f32.powi(i as i32))
        .sum();

    let post_pp = numbers::round((bonus_pp + adjusted_pp).max(0.0) as f32);

    // Accumulate all necessary data
    let content = format!(
        "`{name}`{plural} {mode}top100 {version}:",
        name = user.username,
        plural = plural(user.username.as_str()),
        mode = mode_str(mode),
        version = content_version(version),
    );

    let pages = numbers::div_euclid(5, scores_data.len());
    let pre_pp = user.statistics.as_ref().unwrap().pp;
    let iter = scores_data.iter().take(5);
    let embed_data = TopIfEmbed::new(&user, iter, mode, pre_pp, post_pp, (1, pages)).await;

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // * Don't add maps of scores to DB since their stars were potentially changed

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = TopIfPagination::new(response, user, scores_data, mode, pre_pp, post_pp);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (rebalance): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's top plays on \"upcoming\" pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would look like \
    in an alternative new pp version.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    To use this command, specify the version name **first**, then a username.\n\
    Available versions are:\n  \
    - `xexxar` (see https://github.com/emu1337/osu) [commit 5b80bdb]\n  \
    - `delta` (see https://github.com/HeBuwei/osu) [commit 422d74e]\n  \
    - `sotarks` (see https://sotarks.stanr.info/)\n\
    The translations are not exactly accurate so expect a few differences in the results.
    There are also no guarantees that the implemented versions are up-to-date."
)]
#[usage("[version name] [username]")]
#[example("xexxar badewanne3", "delta \"freddie benson\"", "sotarks peppy")]
pub async fn rebalance(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RebalanceArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rebalance_args)) => {
                    rebalance_args.config.mode.get_or_insert(GameMode::STD);

                    _rebalance(ctx, CommandData::Message { msg, args, num }, rebalance_args).await
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

async fn osu_xexxar(score: &mut Score) -> BotResult<f32> {
    let map_path = prepare_beatmap_file(score.map.as_ref().unwrap().map_id).await?;
    let file = File::open(map_path).await.map_err(PPError::from)?;
    let rosu_map = osu_xexxar::Beatmap::parse(file)
        .await
        .map_err(PPError::from)?;
    let mods = score.mods.bits();

    let max_pp_result = osu_xexxar::OsuPP::new(&rosu_map).mods(mods).calculate();

    let max_pp = max_pp_result.pp();
    score.map.as_mut().unwrap().stars = max_pp_result.stars();

    let pp_result = osu_xexxar::OsuPP::new(&rosu_map)
        .mods(mods)
        .attributes(max_pp_result)
        .n300(score.statistics.count_300 as usize)
        .n100(score.statistics.count_100 as usize)
        .n50(score.statistics.count_50 as usize)
        .misses(score.statistics.count_miss as usize)
        .combo(score.max_combo as usize)
        .calculate();

    score.pp.replace(pp_result.pp());

    Ok(max_pp)
}

async fn osu_delta(score: &mut Score) -> BotResult<f32> {
    let map_path = prepare_beatmap_file(score.map.as_ref().unwrap().map_id).await?;
    let file = File::open(map_path).await.map_err(PPError::from)?;
    let rosu_map = rosu_pp::Beatmap::parse(file).await.map_err(PPError::from)?;
    let mods = score.mods.bits();

    let max_pp_result = osu_delta::OsuPP::new(&rosu_map).mods(mods).calculate();

    let max_pp = max_pp_result.pp();
    score.map.as_mut().unwrap().stars = max_pp_result.stars();

    let pp_result = osu_delta::OsuPP::new(&rosu_map)
        .mods(mods)
        .attributes(max_pp_result)
        .n300(score.statistics.count_300 as usize)
        .n100(score.statistics.count_100 as usize)
        .n50(score.statistics.count_50 as usize)
        .misses(score.statistics.count_miss as usize)
        .combo(score.max_combo as usize)
        .calculate();

    score.pp.replace(pp_result.pp());

    Ok(max_pp)
}

async fn osu_sotarks(score: &mut Score) -> BotResult<f32> {
    let map_path = prepare_beatmap_file(score.map.as_ref().unwrap().map_id).await?;
    let file = File::open(map_path).await.map_err(PPError::from)?;
    let rosu_map = osu_sotarks::Beatmap::parse(file)
        .await
        .map_err(PPError::from)?;
    let mods = score.mods.bits();

    let max_pp_result = osu_sotarks::OsuPP::new(&rosu_map).mods(mods).calculate();

    let max_pp = max_pp_result.pp();
    score.map.as_mut().unwrap().stars = max_pp_result.stars();

    let pp_result = osu_sotarks::OsuPP::new(&rosu_map)
        .mods(mods)
        .attributes(max_pp_result)
        .n300(score.statistics.count_300 as usize)
        .n100(score.statistics.count_100 as usize)
        .n50(score.statistics.count_50 as usize)
        .misses(score.statistics.count_miss as usize)
        .combo(score.max_combo as usize)
        .calculate();

    score.pp.replace(pp_result.pp());

    Ok(max_pp)
}

fn plural(name: &str) -> &'static str {
    match name.chars().last() {
        Some('s') => "'",
        Some(_) | None => "'s",
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::TKO => "taiko ",
        GameMode::CTB => "ctb ",
        GameMode::MNA => "mania ",
    }
}

fn content_version(version: RebalanceVersion) -> &'static str {
    match version {
        RebalanceVersion::Delta => "on the delta_t version",
        RebalanceVersion::Sotarks => "on the Sotarks rebalance",
        RebalanceVersion::Xexxar => "on the Xexxar version",
    }
}

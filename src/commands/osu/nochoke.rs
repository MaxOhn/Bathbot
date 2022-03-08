use std::{cmp::Ordering, sync::Arc};

use eyre::Report;
use futures::stream::{FuturesUnordered, TryStreamExt};
use rosu_pp::{Beatmap as Map, FruitsPP, OsuPP, TaikoPP};
use rosu_v2::prelude::{GameMode, OsuError, Score};
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            ApplicationCommand,
        },
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        parse_discord, parse_mode_option, DoubleResultCow, MyCommand, MyCommandOption,
    },
    custom_client::RankParam,
    database::UserConfig,
    embeds::{EmbedData, NoChokeEmbed},
    error::PpError,
    pagination::{NoChokePagination, Pagination},
    tracking::process_osu_tracking,
    util::{
        constants::{
            common_literals::{CTB, DISCORD, MODE, NAME, OSU, SPECIFY_MODE, TAIKO},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        numbers,
        osu::prepare_beatmap_file,
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

use super::{option_discord, option_name};

async fn _nochokes(ctx: Arc<Context>, data: CommandData<'_>, args: NochokeArgs) -> BotResult<()> {
    let NochokeArgs {
        config,
        miss_limit,
        version,
    } = args;

    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let mut scores_data = match version.calculate(&ctx, scores, miss_limit).await {
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

    let rank_fut = ctx
        .clients
        .custom
        .get_rank_data(mode, RankParam::Pp(unchoked_pp));

    let rank = match rank_fut.await {
        Ok(rank) => Some(rank.rank as usize),
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to get rank pp");
            warn!("{report:?}");

            None
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(5, scores_data.len());
    let embed_data_fut = NoChokeEmbed::new(
        &user,
        scores_data.iter().take(5),
        unchoked_pp,
        rank,
        &ctx,
        (1, pages),
    );
    let embed = embed_data_fut.await.into_builder().build();

    let content = format!(
        "{version} top {mode}scores for `{name}`:",
        version = match version {
            NochokeVersion::Perfect => "Perfect",
            NochokeVersion::Unchoke => "No-choke",
        },
        mode = match mode {
            GameMode::STD => "",
            GameMode::TKO => "taiko ",
            GameMode::CTB => "ctb ",
            GameMode::MNA => panic!("can not unchoke mania scores"),
        },
    );

    // Creating the embed
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = NoChokePagination::new(
        response,
        user,
        scores_data,
        unchoked_pp,
        rank,
        Arc::clone(&ctx),
    );
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 90).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn unchoke_scores(
    ctx: &Context,
    scores: Vec<Score>,
    miss_limit: Option<u32>,
) -> BotResult<Vec<(usize, Score, Score)>> {
    let mut scores_data = Vec::with_capacity(scores.len());

    for (score, i) in scores.into_iter().zip(1..) {
        let map = score.map.as_ref().unwrap();
        let mut unchoked = score.clone();

        let many_misses = miss_limit
            .filter(|&limit| score.statistics.count_miss > limit)
            .is_some();

        // Skip unchoking because it has too many misses or because its a convert
        if many_misses || map.convert {
            scores_data.push((i, score, unchoked));
            continue;
        }

        let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
        let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;
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
                unchoked.pp = Some(pp_result.pp as f32);
                unchoked.grade = unchoked.grade(None);
                unchoked.accuracy = unchoked.accuracy();
            }
            GameMode::CTB if score.max_combo != map.max_combo.unwrap_or(0) => {
                let attributes = rosu_pp::fruits::stars(&rosu_map, mods, None);

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
                unchoked.pp = Some(pp_result.pp as f32);
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

                let pp_result = TaikoPP::new(&rosu_map)
                    .mods(mods)
                    .accuracy(acc as f64)
                    .calculate();

                unchoked.statistics.count_300 = count300 as u32;
                unchoked.statistics.count_100 = count100 as u32;
                unchoked.statistics.count_miss = 0;
                unchoked.max_combo = map.count_circles;
                unchoked.pp = Some(pp_result.pp as f32);
                unchoked.grade = unchoked.grade(Some(acc));
                unchoked.accuracy = unchoked.accuracy();
            }
            GameMode::MNA => bail!("can not unchoke mania scores"),
            _ => {} // Nothing to unchoke
        }

        scores_data.push((i, score, unchoked));
    }

    Ok(scores_data)
}

async fn perfect_scores(
    ctx: &Context,
    scores: Vec<Score>,
    miss_limit: Option<u32>,
) -> BotResult<Vec<(usize, Score, Score)>> {
    scores
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

            let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
            let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;
            let mods = score.mods.bits();
            let total_hits = score.total_hits();

            match map.mode {
                GameMode::STD if score.statistics.count_300 != total_hits => {
                    unchoked.statistics.count_300 = total_hits;
                    unchoked.statistics.count_100 = 0;
                    unchoked.statistics.count_50 = 0;
                    unchoked.statistics.count_miss = 0;

                    let pp_result = OsuPP::new(&rosu_map).mods(mods).calculate();

                    unchoked.max_combo = map
                        .max_combo
                        .unwrap_or_else(|| pp_result.max_combo() as u32);

                    unchoked.pp = Some(pp_result.pp as f32);
                    unchoked.grade = unchoked.grade(Some(100.0));
                    unchoked.accuracy = 100.0;
                }
                GameMode::CTB if (100.0 - score.accuracy).abs() > f32::EPSILON => {
                    let pp_result = FruitsPP::new(&rosu_map).mods(mods).calculate();

                    unchoked.statistics.count_300 = pp_result.difficulty.n_fruits as u32;
                    unchoked.statistics.count_katu = 0;
                    unchoked.statistics.count_100 = pp_result.difficulty.n_droplets as u32;
                    unchoked.statistics.count_50 = pp_result.difficulty.n_tiny_droplets as u32;
                    unchoked.max_combo = pp_result.max_combo() as u32;
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp as f32);
                    unchoked.grade = unchoked.grade(Some(100.0));
                    unchoked.accuracy = 100.0;
                }
                GameMode::TKO if score.statistics.count_miss > 0 => {
                    let pp_result = TaikoPP::new(&rosu_map).mods(mods).calculate();

                    unchoked.statistics.count_300 = map.count_circles;
                    unchoked.statistics.count_100 = 0;
                    unchoked.statistics.count_miss = 0;
                    unchoked.max_combo = map.count_circles;
                    unchoked.pp = Some(pp_result.pp as f32);
                    unchoked.grade = unchoked.grade(Some(100.0));
                    unchoked.accuracy = 100.0;
                }
                GameMode::MNA => bail!("can not unchoke mania scores"),
                _ => {} // Nothing to unchoke
            }

            Ok::<_, Error>((i, score, unchoked))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await
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

#[derive(Copy, Clone)]
pub enum NochokeVersion {
    Perfect,
    Unchoke,
}

impl NochokeVersion {
    async fn calculate(
        self,
        ctx: &Context,
        scores: Vec<Score>,
        miss_limit: Option<u32>,
    ) -> BotResult<Vec<(usize, Score, Score)>> {
        match self {
            NochokeVersion::Perfect => perfect_scores(ctx, scores, miss_limit).await,
            NochokeVersion::Unchoke => unchoke_scores(ctx, scores, miss_limit).await,
        }
    }
}

pub async fn slash_nochoke(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let options = command.yoink_options();

    match NochokeArgs::slash(&ctx, &command, options).await? {
        Ok(args) => _nochokes(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

struct NochokeArgs {
    config: UserConfig,
    miss_limit: Option<u32>,
    version: NochokeVersion,
}

impl NochokeArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;

        if let Some(arg) = args.next() {
            match check_user_mention(ctx, arg).await? {
                Ok(osu) => config.osu = Some(osu),
                Err(content) => return Ok(Err(content)),
            }
        }

        let miss_limit = match args.next().map(str::parse) {
            Some(Ok(num)) => Some(num),
            Some(Err(_)) => {
                let content = "Failed to parse second argument as miss limit.\n\
                    Be sure you specify it as a positive integer.";

                return Ok(Err(content.into()));
            }
            None => None,
        };

        Ok(Ok(Self {
            config,
            miss_limit,
            version: NochokeVersion::Unchoke,
        }))
    }

    async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut miss_limit = None;
        let mut version = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MODE => config.mode = parse_mode_option(&value),
                    "version" => match value.as_str() {
                        "perfect" => version = Some(NochokeVersion::Perfect),
                        "unchoke" => version = Some(NochokeVersion::Unchoke),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => {
                    let number = (option.name == "miss_limit")
                        .then(|| value)
                        .ok_or(Error::InvalidCommandOptions)?;

                    miss_limit = Some(number.max(0) as u32);
                }
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => config.osu = Some(osu),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        Ok(Ok(Self {
            config,
            miss_limit,
            version: version.unwrap_or(NochokeVersion::Unchoke),
        }))
    }
}

pub fn define_nochoke() -> MyCommand {
    let mode_choices = vec![
        CommandOptionChoice::String {
            name: OSU.to_owned(),
            value: OSU.to_owned(),
        },
        CommandOptionChoice::String {
            name: TAIKO.to_owned(),
            value: TAIKO.to_owned(),
        },
        CommandOptionChoice::String {
            name: CTB.to_owned(),
            value: CTB.to_owned(),
        },
    ];

    let mode_help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be unchoked.";

    let mode = MyCommandOption::builder(MODE, SPECIFY_MODE)
        .help(mode_help)
        .string(mode_choices, false);

    let name = option_name();
    let discord = option_discord();

    let miss_limit_description = "Only unchoke scores with at most this many misses";

    let miss_limit = MyCommandOption::builder("miss_limit", miss_limit_description)
        .min_int(0)
        .integer(Vec::new(), false);

    let version_choices = vec![
        CommandOptionChoice::String {
            name: "Unchoke".to_owned(),
            value: "unchoke".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Perfect".to_owned(),
            value: "perfect".to_owned(),
        },
    ];

    let version_help = "Specify a version to unchoke scores.\n\
        - `Unchoke`: Make the score a full combo and transfer all misses to different hitresults. (default)\n\
        - `Perfect`: Make the score a full combo and transfer all misses to the best hitresults.";

    let version = MyCommandOption::builder("version", "Specify a version to unchoke scores")
        .help(version_help)
        .string(version_choices, false);

    let nochoke_description = "How the top plays would look like with only full combos";

    let nochoke_help = "Remove all misses from top scores and make them full combos.\n\
        Then after recalculating their pp, check how many total pp a user could have had.";

    MyCommand::new("nochoke", nochoke_description)
        .help(nochoke_help)
        .options(vec![mode, name, miss_limit, version, discord])
}

use std::{fmt::Write, sync::Arc};

use eyre::Report;
use futures::stream::{FuturesUnordered, TryStreamExt};
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
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
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    pp::{Calculations, PPCalculator},
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MODE, MODS, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher, numbers,
        osu::ModSelection,
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

use super::{option_discord, option_mode, option_name, SortableScore, TopOrder};

const NM: GameMods = GameMods::NoMod;
const DT: GameMods = GameMods::DoubleTime;
const NC: GameMods = GameMods::NightCore;
const HT: GameMods = GameMods::HalfTime;
const EZ: GameMods = GameMods::Easy;
const HR: GameMods = GameMods::HardRock;
const PF: GameMods = GameMods::Perfect;
const SD: GameMods = GameMods::SuddenDeath;

async fn _topif(ctx: Arc<Context>, data: CommandData<'_>, args: IfArgs) -> BotResult<()> {
    let IfArgs { config, mods } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.username() {
        Some(name) => name.as_str(),
        None => return super::require_link(&ctx, &data).await,
    };

    if let ModSelection::Exact(mods) | ModSelection::Include(mods) = mods {
        let mut content = None;
        let ezhr = EZ | HR;
        let dtht = DT | HT;

        if mods & ezhr == ezhr {
            content = Some("Looks like an invalid mod combination, EZ and HR exclude each other.");
        }

        if mods & dtht == dtht {
            content = Some("Looks like an invalid mod combination, DT and HT exclude each other");
        }

        if let Some(content) = content {
            return data.error(&ctx, content).await;
        }
    }

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name, mode);
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
    process_tracking(&ctx, &mut scores, Some(&user)).await;

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|s| s.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;
    let arg_mods = args.mods;

    // Modify scores
    let scores_fut = scores
        .into_iter()
        .enumerate()
        .map(|(i, mut score)| async move {
            let map = score.map.as_ref().unwrap();

            if map.convert {
                return Ok((i + 1, score, None));
            }

            let changed = match arg_mods {
                ModSelection::Exact(mods) => {
                    let changed = score.mods != mods;
                    score.mods = mods;

                    changed
                }
                ModSelection::Exclude(mut mods) if mods != NM => {
                    if mods.contains(DT) {
                        mods |= NC;
                    }

                    if mods.contains(SD) {
                        mods |= PF
                    }

                    let changed = score.mods.intersects(mods);
                    score.mods.remove(mods);

                    changed
                }
                ModSelection::Include(mods) if mods != NM => {
                    let mut changed = false;

                    if mods.contains(DT) && score.mods.contains(HT) {
                        score.mods.remove(HT);
                        changed = true;
                    }

                    if mods.contains(HT) && score.mods.contains(DT) {
                        score.mods.remove(NC);
                        changed = true;
                    }

                    if mods.contains(HR) && score.mods.contains(EZ) {
                        score.mods.remove(EZ);
                        changed = true;
                    }

                    if mods.contains(EZ) && score.mods.contains(HR) {
                        score.mods.remove(HR);
                        changed = true;
                    }

                    changed |= !score.mods.contains(mods);
                    score.mods.insert(mods);

                    changed
                }
                _ => false,
            };

            let mut calculations = Calculations::STARS | Calculations::MAX_PP;

            if changed {
                score.grade = score.grade(Some(score.accuracy));
                calculations |= Calculations::PP;
            }

            let mut calculator = PPCalculator::new().score(&score).map(map);

            calculator.calculate(calculations).await?;

            let max_pp = calculator.max_pp().unwrap_or(0.0);
            let (stars, pp) = (calculator.stars(), calculator.pp());

            drop(calculator);

            if let Some(stars) = stars {
                score.map.as_mut().unwrap().stars = stars;
            }

            if let Some(pp) = pp {
                score.pp.replace(pp);
            }

            Ok((i + 1, score, Some(max_pp)))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect();

    let mut scores_data: Vec<_> = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sort by adjusted pp
    TopOrder::Pp.apply(&mut scores_data).await;

    // Calculate adjusted pp
    let adjusted_pp: f32 = scores_data
        .iter()
        .map(|(i, Score { pp, .. }, ..)| pp.unwrap_or(0.0) * 0.95_f32.powi(*i as i32 - 1))
        .sum();

    let adjusted_pp = numbers::round((bonus_pp + adjusted_pp).max(0.0) as f32);

    let rank_fut = ctx
        .clients
        .custom
        .get_rank_data(mode, RankParam::Pp(adjusted_pp));

    let rank = match rank_fut.await {
        Ok(rank) => Some(rank.rank as usize),
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to get rank pp");
            warn!("{report:?}");

            None
        }
    };

    // Accumulate all necessary data
    let content = get_content(user.username.as_str(), mode, mods);
    let pages = numbers::div_euclid(5, scores_data.len());
    let iter = scores_data.iter().take(5);
    let pre_pp = user.statistics.as_ref().map_or(0.0, |stats| stats.pp);
    let embed_data_fut = TopIfEmbed::new(&user, iter, mode, pre_pp, adjusted_pp, rank, (1, pages));

    // Creating the embed
    let embed = embed_data_fut.await.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // * Don't add maps of scores to DB since their stars were potentially changed

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pre_pp = user.statistics.as_ref().unwrap().pp;
    let pagination =
        TopIfPagination::new(response, user, scores_data, mode, pre_pp, adjusted_pp, rank);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

impl SortableScore for (usize, Score, Option<f32>) {
    fn get(&self) -> &Score {
        &self.1
    }
}

fn get_content(name: &str, mode: GameMode, mods: ModSelection) -> String {
    match mods {
        ModSelection::Exact(mods) => format!(
            "`{name}`{plural} {mode}top100 with only `{mods}` scores:",
            plural = plural(name),
            mode = mode_str(mode),
        ),
        ModSelection::Exclude(mods) if mods != NM => {
            let mods: Vec<_> = mods.iter().collect();
            let len = mods.len();
            let mut mod_iter = mods.into_iter();
            let mut mod_str = String::with_capacity(len * 6 - 2);

            if let Some(first) = mod_iter.next() {
                let last = mod_iter.next_back();
                let _ = write!(mod_str, "`{first}`");

                for elem in mod_iter {
                    let _ = write!(mod_str, ", `{elem}`");
                }

                if let Some(last) = last {
                    let _ = match len {
                        2 => write!(mod_str, " and `{last}`"),
                        _ => write!(mod_str, ", and `{last}`"),
                    };
                }
            }
            format!(
                "`{name}`{plural} {mode}top100 without {mods}:",
                plural = plural(name),
                mode = mode_str(mode),
                mods = mod_str
            )
        }
        ModSelection::Include(mods) if mods != NM => format!(
            "`{name}`{plural} {mode}top100 with `{mods}` inserted everywhere:",
            plural = plural(name),
            mode = mode_str(mode),
        ),
        _ => format!(
            "`{name}`{plural} top {mode}scores:",
            plural = plural(name),
            mode = mode_str(mode),
        ),
    }
}

#[command]
#[short_desc("Display a user's top plays with(out) the given mods")]
#[long_desc(
    "Display how a user's top plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("ti")]
pub async fn topif(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match IfArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut if_args)) => {
                    if_args.config.mode.get_or_insert(GameMode::STD);

                    _topif(ctx, CommandData::Message { msg, args, num }, if_args).await
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
#[short_desc("Display a user's top taiko plays with(out) the given mods")]
#[long_desc(
    "Display how a user's top taiko plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("tit")]
pub async fn topiftaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match IfArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut if_args)) => {
                    if_args.config.mode = Some(GameMode::TKO);

                    _topif(ctx, CommandData::Message { msg, args, num }, if_args).await
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
#[short_desc("Display a user's top ctb plays with(out) the given mods")]
#[long_desc(
    "Display how a user's top ctb plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[example("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("tic")]
pub async fn topifctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match IfArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut if_args)) => {
                    if_args.config.mode = Some(GameMode::CTB);

                    _topif(ctx, CommandData::Message { msg, args, num }, if_args).await
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

pub async fn slash_topif(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let options = command.yoink_options();

    match IfArgs::slash(&ctx, &command, options).await? {
        Ok(args) => _topif(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

struct IfArgs {
    config: UserConfig,
    mods: ModSelection,
}

impl IfArgs {
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want to insert mods everywhere, specify it e.g. as `+hrdt`.\n\
        If you want to replace mods everywhere, specify it e.g. as `+hdhr!`.\n\
        And if you want to remote mods everywhere, specify it e.g. as `-hdnf!`.";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut mods = None;

        for arg in args.take(2) {
            match matcher::get_mods(arg) {
                Some(mods_) => mods = Some(mods_),
                None => match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                },
            }
        }

        let mods = match mods {
            Some(mods_) => mods_,
            None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
        };

        Ok(Ok(Self { config, mods }))
    }

    async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut mods = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
                    MODE => config.mode = parse_mode_option(&value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
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

        let mods = mods.ok_or(Error::InvalidCommandOptions)?;

        Ok(Ok(Self { config, mods }))
    }
}

pub fn define_topif() -> MyCommand {
    let mode = option_mode();

    let mods_description =
        "Specify mods (`+mods` to insert them, `+mods!` to replace, `-mods!` to remove)";

    let mods_help = "Specify how the top score mods should be adjusted.\n\
        Mods must be given as `+mods` to included them everywhere, `+mods!` to replace them exactly, \
        or `-mods!` to excluded them everywhere.\n\
        Examples:\n\
        - `+hd`: Add `HD` to all scores\n\
        - `+hdhr!`: Make all scores `HDHR` scores\n\
        - `+nm!`: Make all scores nomod scores\n\
        - `-ezhd!`: Remove both `EZ` and `HD` from all scores";

    let mods = MyCommandOption::builder(MODS, mods_description)
        .help(mods_help)
        .string(Vec::new(), true);

    let name = option_name();
    let discord = option_discord();

    let if_description = "How the top plays would look like with different mods";

    MyCommand::new("topif", if_description).options(vec![mods, mode, name, discord])
}

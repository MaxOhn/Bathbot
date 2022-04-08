use std::{borrow::Cow, fmt::Write, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::Report;
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    custom_client::RankParam,
    embeds::{EmbedData, TopIfEmbed},
    pagination::{Pagination, TopIfPagination},
    pp::PpCalculator,
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, numbers,
        osu::{ModSelection, ScoreOrder},
        query::FilterCriteria,
        ApplicationCommandExt,
    },
    BotResult, Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(name = "topif")]
/// How the top plays would look like with different mods
pub struct TopIf<'a> {
    #[command(help = "Specify how the top score mods should be adjusted.\n\
        Mods must be given as `+mods` to included them everywhere, `+mods!` to replace them exactly, \
        or `-mods!` to excluded them everywhere.\n\
        Examples:\n\
        - `+hd`: Add `HD` to all scores\n\
        - `+hdhr!`: Make all scores `HDHR` scores\n\
        - `+nm!`: Make all scores nomod scores\n\
        - `-ezhd!`: Remove both `EZ` and `HD` from all scores")]
    /// Specify mods (`+mods` to insert them, `+mods!` to replace, `-mods!` to remove)
    mods: Cow<'a, str>,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    /// Specify a search query containing artist, difficulty, AR, BPM, ...
    query: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

async fn slash_topif(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = TopIf::from_interaction(command.input_data())?;

    topif(ctx, command.into(), args).await
}

impl<'m> TopIf<'m> {
    fn args(mode: GameMode, args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;
        let mut mods = None;

        for arg in args.take(2) {
            if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            mods,
            mode: Some(mode),
            name,
            query: None,
            discord,
        }
    }
}

#[command]
#[desc("Display a user's top plays with(out) the given mods")]
#[help(
    "Display how a user's top plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[examples("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[alias("ti")]
#[group(Osu)]
async fn prefix_topif(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    topif(ctx, msg.into(), TopIf::args(GameMode::STD, args))
}

#[command]
#[desc("Display a user's top taiko plays with(out) the given mods")]
#[help(
    "Display how a user's top taiko plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[examples("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[alias("tit")]
#[group(Taiko)]
async fn prefix_topiftaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    topif(ctx, msg.into(), TopIf::args(GameMode::TKO, args))
}

#[command]
#[desc("Display a user's top ctb plays with(out) the given mods")]
#[help(
    "Display how a user's top ctb plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[examples("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[alias("tic")]
#[group(Catch)]
async fn prefix_topifctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    topif(ctx, msg.into(), TopIf::args(GameMode::CTB, args))
}

const NM: GameMods = GameMods::NoMod;
const DT: GameMods = GameMods::DoubleTime;
const NC: GameMods = GameMods::NightCore;
const HT: GameMods = GameMods::HalfTime;
const EZ: GameMods = GameMods::Easy;
const HR: GameMods = GameMods::HardRock;
const PF: GameMods = GameMods::Perfect;
const SD: GameMods = GameMods::SuddenDeath;

async fn topif(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: TopIf<'_>) -> BotResult<()> {
    let mods = match matcher::get_mods(&args.mods) {
        Some(mods) => mods,
        None => {
            let content = "Failed to parse mods.\n\
            If you want to insert mods everywhere, specify it e.g. as `+hrdt`.\n\
            If you want to replace mods everywhere, specify it e.g. as `+hdhr!`.\n\
            And if you want to remove mods everywhere, specify it e.g. as `-hdnf!`.";

            return orig.error(&ctx, content).await;
        }
    };

    let (name, mode) = name_mode!(ctx, orig, args);

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
            return orig.error(&ctx, content).await;
        }
    }

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(&name, mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|s| s.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;
    let arg_mods = args.mods;

    let mut scores_data: Vec<_> = match modify_scores(&ctx, scores, arg_mods).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    // Sort by adjusted pp
    ScoreOrder::Pp.apply(&ctx, &mut scores_data).await;

    // Calculate adjusted pp
    let adjusted_pp: f32 = scores_data
        .iter()
        .map(|(i, Score { pp, .. }, _)| pp.unwrap_or(0.0) * 0.95_f32.powi(*i as i32 - 1))
        .sum();

    if let Some(query) = args.query.as_deref() {
        let criteria = FilterCriteria::new(query);

        scores_data.retain(|(_, score, _)| score.matches(&criteria));
    }

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
    let content = get_content(user.username.as_str(), mode, mods, args.query.as_deref());
    let pages = numbers::div_euclid(5, scores_data.len());
    let iter = scores_data.iter().take(5);
    let pre_pp = user.statistics.as_ref().map_or(0.0, |stats| stats.pp);
    let embed_data_fut = TopIfEmbed::new(&user, iter, mode, pre_pp, adjusted_pp, rank, (1, pages));

    // Creating the embed
    let embed = embed_data_fut.await.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = orig.create_message(&ctx, &builder).await?;

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
    let owner = orig.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn modify_scores(
    ctx: &Context,
    scores: Vec<Score>,
    arg_mods: ModSelection,
) -> BotResult<Vec<(usize, Score, Option<f32>)>> {
    let mut scores_data = Vec::with_capacity(scores.len());

    for (mut score, i) in scores.into_iter().zip(1..) {
        let map = score.map.as_ref().unwrap();

        if map.convert {
            scores_data.push((i, score, None));
            continue;
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

        if changed {
            score.grade = score.grade(Some(score.accuracy));
        }

        let mut calc = PpCalculator::new(ctx, map.map_id).await?;
        calc.score(&score);

        let stars = calc.stars() as f32;
        let max_pp = calc.max_pp() as f32;

        let pp = if let Some(pp) = score.pp.filter(|_| !changed) {
            pp
        } else {
            calc.pp() as f32
        };

        drop(calc);

        score.map.as_mut().unwrap().stars = stars;
        score.pp = Some(pp);

        scores_data.push((i, score, Some(max_pp)));
    }

    Ok(scores_data)
}

fn get_content(name: &str, mode: GameMode, mods: ModSelection, query: Option<&str>) -> String {
    let mut content = match mods {
        ModSelection::Exact(mods) => format!(
            "`{name}`{plural} {mode}top100 with only `{mods}` scores",
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
                "`{name}`{plural} {mode}top100 without {mods}",
                plural = plural(name),
                mode = mode_str(mode),
                mods = mod_str
            )
        }
        ModSelection::Include(mods) if mods != NM => format!(
            "`{name}`{plural} {mode}top100 with `{mods}` inserted everywhere",
            plural = plural(name),
            mode = mode_str(mode),
        ),
        _ => format!(
            "`{name}`{plural} top {mode}scores",
            plural = plural(name),
            mode = mode_str(mode),
        ),
    };

    if let Some(query) = query {
        let _ = write!(content, " (`Query: {query}`):");
    } else {
        content.push(':');
    }

    content
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

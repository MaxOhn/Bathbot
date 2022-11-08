use std::{borrow::Cow, cmp::Ordering, fmt::Write, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{osu::user_not_found, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    manager::{redis::osu::UserArgs, OsuMap},
    pagination::TopIfPagination,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        matcher,
        numbers::round,
        osu::{ModSelection, ScoreSlim},
        query::{FilterCriteria, Searchable},
        ChannelExt, InteractionCommandExt,
    },
    Context,
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

async fn slash_topif(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = TopIf::from_interaction(command.input_data())?;

    topif(ctx, (&mut command).into(), args).await
}

impl<'m> TopIf<'m> {
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want add mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to remove mods, specify it e.g. as `-hdnf!`.";

    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, &'static str> {
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

        Ok(Self {
            mods: mods.ok_or(Self::ERR_PARSE_MODS)?,
            mode,
            name,
            query: None,
            discord,
        })
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
async fn prefix_topif(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TopIf::args(None, args) {
        Ok(args) => topif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
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
async fn prefix_topiftaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TopIf::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => topif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
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
#[aliases("tic", "topifcatch")]
#[group(Catch)]
async fn prefix_topifctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TopIf::args(Some(GameModeOption::Catch), args) {
        Ok(args) => topif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

const NM: GameMods = GameMods::NoMod;
const DT: GameMods = GameMods::DoubleTime;
const NC: GameMods = GameMods::NightCore;
const HT: GameMods = GameMods::HalfTime;
const EZ: GameMods = GameMods::Easy;
const HR: GameMods = GameMods::HardRock;
const PF: GameMods = GameMods::Perfect;
const SD: GameMods = GameMods::SuddenDeath;

async fn topif(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: TopIf<'_>) -> Result<()> {
    let mods = match matcher::get_mods(&args.mods) {
        Some(mods) => mods,
        None => return orig.error(&ctx, TopIf::ERR_PARSE_MODS).await,
    };

    let (user_id, mut mode) = user_id_mode!(ctx, orig, args);

    if mode == GameMode::Mania {
        mode = GameMode::Osu;
    }

    if let Err(content) = mods.validate() {
        return orig.error(&ctx, content).await;
    }

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);
    let scores_fut = ctx.osu_scores().top().limit(100).exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|s| s.weight)
        .fold(0.0, |sum, weight| sum + weight.pp);

    let bonus_pp = user.peek_stats(|stats| stats.pp - actual_pp);

    let mut entries = match process_scores(&ctx, scores, mods).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to modify scores"));
        }
    };

    // Sort by adjusted pp
    entries.sort_unstable_by(|a, b| {
        b.score
            .pp
            .partial_cmp(&a.score.pp)
            .unwrap_or(Ordering::Equal)
    });

    // Calculate adjusted pp
    let adjusted_pp: f32 = entries.iter().zip(0..).fold(0.0, |sum, (entry, i)| {
        sum + entry.score.pp * 0.95_f32.powi(i)
    });

    if let Some(query) = args.query.as_deref() {
        let criteria = FilterCriteria::new(query);

        entries.retain(|entry| entry.matches(&criteria));
    }

    let final_pp = round(bonus_pp + adjusted_pp);

    let rank = match ctx.approx().rank(final_pp, mode).await {
        Ok(rank) => Some(rank),
        Err(err) => {
            warn!("{:?}", err.wrap_err("failed to get rank from pp"));

            None
        }
    };

    // Accumulate all necessary data
    let pre_pp = user.peek_stats(|stats| stats.pp);
    let content = get_content(user.username(), mode, mods, args.query.as_deref());

    TopIfPagination::builder(user, entries, mode, pre_pp, final_pp, rank)
        .content(content)
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

pub struct TopIfEntry {
    pub original_idx: usize,
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub stars: f32,
    pub max_pp: f32,
}

async fn process_scores(
    ctx: &Context,
    scores: Vec<Score>,
    arg_mods: ModSelection,
) -> Result<Vec<TopIfEntry>> {
    let mut entries = Vec::with_capacity(scores.len());

    let maps_id_checksum = scores
        .iter()
        .filter_map(|score| score.map.as_ref())
        .map(|map| (map.map_id as i32, map.checksum.as_deref()))
        .collect();

    let mut maps = ctx.osu_map().maps(&maps_id_checksum).await?;

    for (mut score, i) in scores.into_iter().zip(1..) {
        let map = score
            .map
            .as_ref()
            .and_then(|map| maps.remove(&map.map_id))
            .expect("missing map");

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

        let mut calc = ctx.pp(&map).mode(score.mode).mods(score.mods);
        let attrs = calc.performance().await;

        let pp = if let Some(pp) = score.pp.filter(|_| !changed) {
            pp
        } else {
            calc.score(&score).performance().await.pp() as f32
        };

        let entry = TopIfEntry {
            original_idx: i,
            score: ScoreSlim::new(score, pp),
            map,
            stars: attrs.stars() as f32,
            max_pp: attrs.pp() as f32,
        };

        entries.push(entry);
    }

    Ok(entries)
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
        GameMode::Osu => "",
        GameMode::Taiko => "taiko ",
        GameMode::Catch => "ctb ",
        GameMode::Mania => "mania ",
    }
}

use std::{borrow::Cow, cmp::Ordering, fmt::Write, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    numbers::round,
    osu::ModSelection,
};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameModIntermode, GameMode, GameMods, GameModsIntermode, OsuError, Score};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    active::{impls::TopIfPagination, ActiveMessages},
    commands::{osu::user_not_found, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    manager::{redis::osu::UserArgs, OsuMap},
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, Searchable},
        ChannelExt, InteractionCommandExt,
    },
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "topif",
    desc = "How the top plays would look like with different mods"
)]
pub struct TopIf<'a> {
    #[command(
        desc = "Specify mods (`+mods` to insert them, `+mods!` to replace, `-mods!` to remove)",
        help = "Specify how the top score mods should be adjusted.\n\
        Mods must be given as `+mods` to included them everywhere, `+mods!` to replace them exactly, \
        or `-mods!` to excluded them everywhere.\n\
        Examples:\n\
        - `+hd`: Add `HD` to all scores\n\
        - `+hdhr!`: Make all scores `HDHR` scores\n\
        - `+nm!`: Make all scores nomod scores\n\
        - `-ezhd!`: Remove both `EZ` and `HD` from all scores"
    )]
    mods: Cow<'a, str>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    query: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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

async fn topif(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: TopIf<'_>) -> Result<()> {
    let mods = match matcher::get_mods(&args.mods) {
        Some(mods) => mods,
        None => return orig.error(&ctx, TopIf::ERR_PARSE_MODS).await,
    };

    let (user_id, mut mode) = user_id_mode!(ctx, orig, args);

    if mode == GameMode::Mania {
        mode = GameMode::Osu;
    }

    if let Err(content) = mods.clone().validate(mode) {
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

    let bonus_pp = user.stats().pp() - actual_pp;
    let content = get_content(user.username(), mode, &mods, args.query.as_deref());

    let mut entries = match process_scores(&ctx, scores, mods, mode).await {
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
            warn!(?err, "Failed to get rank from pp");

            None
        }
    };

    // Accumulate all necessary data
    let pre_pp = user.stats().pp();

    let pagination = TopIfPagination::builder()
        .user(user)
        .entries(entries.into_boxed_slice())
        .mode(mode)
        .pre_pp(pre_pp)
        .post_pp(final_pp)
        .rank(rank)
        .content(content.into_boxed_str())
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}

pub struct TopIfEntry {
    pub original_idx: usize,
    pub old_pp: f32,
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub stars: f32,
    pub max_pp: f32,
    pub max_combo: u32,
}

async fn process_scores(
    ctx: &Context,
    scores: Vec<Score>,
    mut arg_mods: ModSelection,
    mode: GameMode,
) -> Result<Vec<TopIfEntry>> {
    let mut entries = Vec::with_capacity(scores.len());

    let maps_id_checksum = scores
        .iter()
        .map(|score| {
            (
                score.map_id as i32,
                score.map.as_ref().and_then(|map| map.checksum.as_deref()),
            )
        })
        .collect();

    let mut maps = ctx.osu_map().maps(&maps_id_checksum).await?;

    match &mut arg_mods {
        ModSelection::Exact(mods) | ModSelection::Include(mods) if mods.is_empty() => {
            *mods = GameModsIntermode::new();
        }
        ModSelection::Exclude(mods) => {
            if mods.contains(GameModIntermode::DoubleTime) {
                *mods |= GameModIntermode::Nightcore;
            }

            if mods.contains(GameModIntermode::SuddenDeath) {
                *mods |= GameModIntermode::Perfect;
            }
        }
        ModSelection::Exact(_) | ModSelection::Include(_) => {}
    }

    let converted_mods = arg_mods
        .as_mods()
        .to_owned()
        .with_mode(mode)
        .expect("mods have been validated before");

    for (mut score, i) in scores.into_iter().zip(1..) {
        let Some(mut map) = maps.remove(&score.map_id) else { continue };
        map = map.convert(score.mode);

        let changed = match &arg_mods {
            ModSelection::Include(mods) if mods.is_empty() => {
                let changed = !score.mods.is_empty();
                score.mods = GameMods::new();

                changed
            }
            ModSelection::Exact(_) => {
                let changed = score.mods != converted_mods;
                score.mods = converted_mods.clone();

                changed
            }
            ModSelection::Exclude(mods) => {
                let changed = score.mods.contains_any(mods.iter());
                score.mods.remove_all_intermode(mods.iter());

                changed
            }
            ModSelection::Include(mods) => {
                let mut changed = false;

                if mods.contains(GameModIntermode::DoubleTime)
                    || mods.contains(GameModIntermode::Nightcore)
                {
                    changed |= score.mods.remove_intermode(GameModIntermode::HalfTime);
                }

                if mods.contains(GameModIntermode::HalfTime) {
                    changed |= score.mods.remove_intermode(GameModIntermode::DoubleTime);
                    changed |= score.mods.remove_intermode(GameModIntermode::Nightcore);
                }

                if mods.contains(GameModIntermode::HardRock) {
                    changed |= score.mods.remove_intermode(GameModIntermode::Easy);
                }

                if mods.contains(GameModIntermode::Easy) {
                    changed |= score.mods.remove_intermode(GameModIntermode::HardRock);
                }

                changed |= !mods
                    .iter()
                    .all(|gamemod| score.mods.contains_intermode(gamemod));

                score.mods.extend(converted_mods.iter().cloned());

                changed
            }
        };

        if changed {
            score.grade = score.grade(Some(score.accuracy));
        }

        let mut calc = ctx.pp(&map).mode(score.mode).mods(score.mods.bits());
        let attrs = calc.performance().await;

        let old_pp = score.pp.expect("missing pp");

        let new_pp = if changed {
            calc.score(&score).performance().await.pp() as f32
        } else {
            old_pp
        };

        let entry = TopIfEntry {
            original_idx: i,
            score: ScoreSlim::new(score, new_pp),
            old_pp,
            map,
            stars: attrs.stars() as f32,
            max_pp: attrs.pp() as f32,
            max_combo: attrs.max_combo() as u32,
        };

        entries.push(entry);
    }

    Ok(entries)
}

fn get_content(name: &str, mode: GameMode, mods: &ModSelection, query: Option<&str>) -> String {
    let mut content = match mods {
        ModSelection::Exact(mods) => format!(
            "`{name}`{plural} {mode}top100 with only `{mods}` scores",
            plural = plural(name),
            mode = mode_str(mode),
        ),
        ModSelection::Exclude(mods) if !mods.is_empty() => {
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
        ModSelection::Include(mods) if !mods.is_empty() => format!(
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

use std::{borrow::Cow, cmp::Ordering, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::calculate_grade,
};
use eyre::{Report, Result};
use rosu_pp::{BeatmapExt, DifficultyAttributes, GameMode as Mode};
use rosu_v2::prelude::{GameMode, GameMods, Grade, OsuError, Score, ScoreStatistics};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    manager::{redis::osu::UserArgs, OsuMap},
    pagination::NoChokePagination,
    util::{interaction::InteractionCommand, osu::IfFc, InteractionCommandExt},
    Context,
};

use super::user_not_found;

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "nochoke",
    help = "Remove all misses from top scores and make them full combos.\n\
    Then after recalculating their pp, check how many total pp a user could have had."
)]
/// How the top plays would look like with only full combos
pub struct Nochoke<'a> {
    #[command(help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be unchoked.")]
    /// Specify a gamemode
    mode: Option<NochokeGameMode>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(min_value = 0)]
    /// Only unchoke scores with at most this many misses
    miss_limit: Option<u32>,
    #[command(help = "Specify a version to unchoke scores.\n\
        - `Unchoke`: Make the score a full combo and transfer all misses to different hitresults. (default)\n\
        - `Perfect`: Make the score a full combo and transfer all misses to the best hitresults.")]
    /// Specify a version to unchoke scores
    version: Option<NochokeVersion>,
    /// Filter out certain scores
    filter: Option<NochokeFilter>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum NochokeGameMode {
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
}

impl From<NochokeGameMode> for GameMode {
    #[inline]
    fn from(mode: NochokeGameMode) -> Self {
        match mode {
            NochokeGameMode::Osu => Self::Osu,
            NochokeGameMode::Taiko => Self::Taiko,
            NochokeGameMode::Catch => Self::Catch,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum NochokeVersion {
    #[option(name = "Unchoke", value = "unchoke")]
    Unchoke,
    #[option(name = "Perfect", value = "perfect")]
    Perfect,
}

impl Default for NochokeVersion {
    #[inline]
    fn default() -> Self {
        Self::Unchoke
    }
}

#[derive(CommandOption, CreateOption)]
pub enum NochokeFilter {
    #[option(name = "Only keep chokes", value = "only_chokes")]
    OnlyChokes,
    #[option(name = "Remove all chokes", value = "remove_chokes")]
    RemoveChokes,
}

impl<'m> Nochoke<'m> {
    fn args(mode: Option<NochokeGameMode>, args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;
        let mut miss_limit = None;

        for arg in args.take(2) {
            if let Ok(num) = arg.parse() {
                miss_limit = Some(num);
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            mode,
            name,
            miss_limit,
            version: None,
            filter: None,
            discord,
        }
    }
}

#[command]
#[desc("Unchoke a user's top100")]
#[help(
    "Display a user's top plays if no score in their top100 would be a choke.\n
    If a number is specified, I will only unchoke scores with at most that many misses"
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[aliases("nc", "nochoke")]
#[group(Osu)]
async fn prefix_nochokes(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(None, args);

    nochoke(ctx, msg.into(), args).await
}

#[command]
#[desc("Unchoke a user's taiko top100")]
#[help(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[alias("nct", "nochoketaiko")]
#[group(Taiko)]
async fn prefix_nochokestaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(Some(NochokeGameMode::Taiko), args);

    nochoke(ctx, msg.into(), args).await
}

#[command]
#[desc("Unchoke a user's ctb top100")]
#[help(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[alias("ncc", "nochokectb", "nochokecatch", "nochokescatch")]
#[group(Catch)]
async fn prefix_nochokesctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(Some(NochokeGameMode::Catch), args);

    nochoke(ctx, msg.into(), args).await
}

async fn slash_nochoke(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Nochoke::from_interaction(command.input_data())?;

    nochoke(ctx, (&mut command).into(), args).await
}

async fn nochoke(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Nochoke<'_>) -> Result<()> {
    let (user_id, mut mode) = user_id_mode!(ctx, orig, args);

    if mode == GameMode::Mania {
        mode = GameMode::Osu;
    }

    let Nochoke {
        miss_limit,
        version,
        filter,
        ..
    } = args;

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

    let version = version.unwrap_or_default();

    let mut entries = match process_scores(&ctx, scores, miss_limit, version).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to process scores"));
        }
    };

    // Calculate bonus pp
    let actual_pp: f32 = entries
        .iter()
        .map(|entry| entry.original_score.pp)
        .zip(0..)
        .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

    let bonus_pp = user.stats().pp() - actual_pp;

    // Sort by unchoked pp
    entries.sort_unstable_by(|a, b| {
        b.unchoked_pp()
            .partial_cmp(&a.unchoked_pp())
            .unwrap_or(Ordering::Equal)
    });

    // Calculate total user pp without chokes
    let mut unchoked_pp: f32 = entries
        .iter()
        .map(NochokeEntry::unchoked_pp)
        .zip(0..)
        .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

    unchoked_pp = (100.0 * (unchoked_pp + bonus_pp)).round() / 100.0;

    match filter {
        Some(NochokeFilter::OnlyChokes) => entries.retain(|entry| entry.unchoked.is_some()),
        Some(NochokeFilter::RemoveChokes) => entries.retain(|entry| entry.unchoked.is_none()),
        None => {}
    }

    let rank = match ctx.approx().rank(unchoked_pp, mode).await {
        Ok(rank) => Some(rank),
        Err(err) => {
            warn!("{:?}", err.wrap_err("failed to get rank pp"));

            None
        }
    };

    let mut content = format!(
        "{version} top {mode}scores for `{name}`",
        version = match version {
            NochokeVersion::Perfect => "Perfect",
            NochokeVersion::Unchoke => "No-choke",
        },
        mode = match mode {
            GameMode::Osu => "",
            GameMode::Taiko => "taiko ",
            GameMode::Catch => "ctb ",
            GameMode::Mania => "mania ",
        },
        name = user.username(),
    );

    match filter {
        Some(NochokeFilter::OnlyChokes) => content.push_str(" (only chokes)"),
        Some(NochokeFilter::RemoveChokes) => content.push_str(" (removed chokes)"),
        None => {}
    }

    content.push(':');

    NoChokePagination::builder(user, entries, unchoked_pp, rank)
        .content(content)
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

pub struct NochokeEntry {
    pub original_idx: usize,
    pub original_score: ScoreSlim,
    pub unchoked: Option<Unchoked>,
    pub map: OsuMap,
    pub max_pp: f32,
    pub stars: f32,
    pub max_combo: u32,
}

impl NochokeEntry {
    pub fn unchoked_pp(&self) -> f32 {
        self.unchoked
            .as_ref()
            .map_or(self.original_score.pp, |unchoked| unchoked.pp)
    }

    pub fn unchoked_grade(&self) -> Grade {
        self.unchoked
            .as_ref()
            .map_or(self.original_score.grade, |unchoked| unchoked.grade)
    }

    pub fn unchoked_max_combo(&self) -> u32 {
        if self.unchoked.is_some() {
            self.max_combo
        } else {
            self.original_score.max_combo
        }
    }

    pub fn unchoked_statistics(&self) -> &ScoreStatistics {
        self.unchoked
            .as_ref()
            .map_or(&self.original_score.statistics, |unchoked| {
                &unchoked.statistics
            })
    }

    pub fn unchoked_accuracy(&self) -> f32 {
        self.unchoked
            .as_ref()
            .map_or(self.original_score.accuracy, |unchoked| {
                unchoked.statistics.accuracy(self.original_score.mode)
            })
    }
}

pub struct Unchoked {
    pub grade: Grade,
    pub pp: f32,
    pub statistics: ScoreStatistics,
}

impl Unchoked {
    fn new(if_fc: IfFc, mods: GameMods, mode: GameMode) -> Self {
        let grade = calculate_grade(mode, mods, &if_fc.statistics);

        Self {
            grade,
            pp: if_fc.pp,
            statistics: if_fc.statistics,
        }
    }
}

async fn process_scores(
    ctx: &Context,
    scores: Vec<Score>,
    miss_limit: Option<u32>,
    version: NochokeVersion,
) -> Result<Vec<NochokeEntry>> {
    let mut entries = Vec::with_capacity(scores.len());

    let maps_id_checksum = scores
        .iter()
        .filter_map(|score| score.map.as_ref())
        .map(|map| (map.map_id as i32, map.checksum.as_deref()))
        .collect();

    let mut maps = ctx.osu_map().maps(&maps_id_checksum).await?;
    let miss_limit = miss_limit.unwrap_or(u32::MAX);

    for (i, score) in scores.into_iter().enumerate() {
        let Some(mut map) = maps.remove(&score.map_id) else { continue };
        map = map.convert(score.mode);

        let attrs = ctx
            .pp(&map)
            .mode(score.mode)
            .mods(score.mods)
            .performance()
            .await;

        let pp = score.pp.expect("missing pp");

        let max_pp = if score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania && pp > 0.0
        {
            pp
        } else {
            attrs.pp() as f32
        };

        let score = ScoreSlim::new(score, pp);
        let too_many_misses = score.statistics.count_miss > miss_limit;

        let unchoked = match version {
            NochokeVersion::Unchoke if too_many_misses => None,
            // Skip unchoking because it has too many misses or because its a convert
            NochokeVersion::Unchoke => IfFc::new(ctx, &score, &map)
                .await
                .map(|if_fc| Unchoked::new(if_fc, score.mods, score.mode)),
            NochokeVersion::Perfect if too_many_misses => None,
            NochokeVersion::Perfect => Some(perfect_score(ctx, &score, &map).await),
        };

        let entry = NochokeEntry {
            original_idx: i,
            original_score: score,
            unchoked,
            map,
            max_pp,
            stars: attrs.stars() as f32,
            max_combo: attrs.max_combo() as u32,
        };

        entries.push(entry);
    }

    Ok(entries)
}

async fn perfect_score(ctx: &Context, score: &ScoreSlim, map: &OsuMap) -> Unchoked {
    let total_hits = score.total_hits();
    let mut calc = ctx.pp(map).mode(score.mode).mods(score.mods);
    let attrs = calc.difficulty().await;

    let stats = match attrs {
        DifficultyAttributes::Osu(_) if score.statistics.count_300 != total_hits => {
            ScoreStatistics {
                count_geki: 0,
                count_300: total_hits,
                count_katu: 0,
                count_100: 0,
                count_50: 0,
                count_miss: 0,
            }
        }
        DifficultyAttributes::Taiko(_) if score.statistics.count_miss > 0 => ScoreStatistics {
            count_geki: 0,
            count_300: map.n_circles() as u32,
            count_katu: 0,
            count_100: 0,
            count_50: 0,
            count_miss: 0,
        },
        DifficultyAttributes::Catch(attrs) if (100.0 - score.accuracy).abs() > f32::EPSILON => {
            ScoreStatistics {
                count_geki: 0,
                count_300: attrs.n_fruits as u32,
                count_katu: 0,
                count_100: attrs.n_droplets as u32,
                count_50: attrs.n_tiny_droplets as u32,
                count_miss: 0,
            }
        }
        DifficultyAttributes::Mania(_) if score.statistics.count_geki != total_hits => {
            ScoreStatistics {
                count_geki: total_hits,
                count_300: 0,
                count_katu: 0,
                count_100: 0,
                count_50: 0,
                count_miss: 0,
            }
        }
        _ => score.statistics.clone(), // Nothing to unchoke
    };

    let grade = calculate_grade(score.mode, score.mods, &stats);

    let mode = match score.mode {
        GameMode::Osu => Mode::Osu,
        GameMode::Taiko => Mode::Taiko,
        GameMode::Catch => Mode::Catch,
        GameMode::Mania => Mode::Mania,
    };

    let pp = map
        .pp_map
        .pp()
        .attributes(attrs.to_owned())
        .mods(score.mods.bits())
        .mode(mode)
        .n_geki(stats.count_geki as usize)
        .n300(stats.count_300 as usize)
        .n_katu(stats.count_katu as usize)
        .n100(stats.count_100 as usize)
        .n50(stats.count_50 as usize)
        .n_misses(0)
        .calculate()
        .pp() as f32;

    Unchoked {
        grade,
        pp,
        statistics: stats,
    }
}

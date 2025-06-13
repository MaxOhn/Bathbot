use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand, command};
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{constants::GENERAL_ISSUE, matcher, osu::calculate_grade};
use eyre::{Report, Result};
use rosu_pp::any::DifficultyAttributes;
use rosu_v2::{
    prelude::{GameMode, GameMods, Grade, OsuError, Score, ScoreStatistics},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{Id, marker::UserMarker};

use super::{require_link, user_not_found};
use crate::{
    Context,
    active::{ActiveMessages, impls::NoChokePagination},
    commands::{
        DISCORD_OPTION_DESC, DISCORD_OPTION_HELP,
        utility::{SCORE_DATA_DESC, SCORE_DATA_HELP},
    },
    core::commands::{CommandOrigin, prefix::Args},
    manager::{
        OsuMap,
        redis::osu::{UserArgs, UserArgsError},
    },
    util::{InteractionCommandExt, interaction::InteractionCommand, osu::IfFc},
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "nochoke",
    desc = "How the top plays would look like with only full combos",
    help = "Remove all misses from top scores and make them full combos.\n\
    Then after recalculating their pp, check how many total pp a user could have had."
)]
pub struct Nochoke<'a> {
    #[command(
        desc = "Specify a gamemode",
        help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be unchoked."
    )]
    mode: Option<NochokeGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 0,
        desc = "Only unchoke scores with at most this many misses"
    )]
    miss_limit: Option<u32>,
    #[command(
        desc = "Specify a version to unchoke scores",
        help = "Specify a version to unchoke scores.\n\
        - `Unchoke`: Make the score a full combo and transfer all misses to different hitresults. (default)\n\
        - `Perfect`: Make the score a full combo and transfer all misses to the best hitresults."
    )]
    version: Option<NochokeVersion>,
    #[command(desc = "Filter out certain scores")]
    filter: Option<NochokeFilter>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
    #[command(desc = SCORE_DATA_DESC, help = SCORE_DATA_HELP)]
    score_data: Option<ScoreData>,
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
            score_data: None,
        }
    }
}

#[command]
#[desc("Unchoke a user's top200")]
#[help(
    "Display a user's top plays if no score in their top200 would be a choke.\n
    If a number is specified, I will only unchoke scores with at most that many misses"
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[aliases("nc", "nochoke")]
#[group(Osu)]
async fn prefix_nochokes(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(None, args);

    nochoke(msg.into(), args).await
}

#[command]
#[desc("Unchoke a user's taiko top200")]
#[help(
    "Display a user's top plays if no score in their top200 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[alias("nct", "nochoketaiko")]
#[group(Taiko)]
async fn prefix_nochokestaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(Some(NochokeGameMode::Taiko), args);

    nochoke(msg.into(), args).await
}

#[command]
#[desc("Unchoke a user's ctb top200")]
#[help(
    "Display a user's top plays if no score in their top200 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[alias("ncc", "nochokectb", "nochokecatch", "nochokescatch")]
#[group(Catch)]
async fn prefix_nochokesctb(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(Some(NochokeGameMode::Catch), args);

    nochoke(msg.into(), args).await
}

async fn slash_nochoke(mut command: InteractionCommand) -> Result<()> {
    let args = Nochoke::from_interaction(command.input_data())?;

    nochoke((&mut command).into(), args).await
}

async fn nochoke(orig: CommandOrigin<'_>, args: Nochoke<'_>) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let mode = match args.mode.map(GameMode::from).or(config.mode) {
        None | Some(GameMode::Mania) => GameMode::Osu,
        Some(mode) => mode,
    };

    let legacy_scores = match args.score_data.or(config.score_data) {
        Some(score_data) => score_data.is_legacy(),
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data)
                .await
                .is_some_and(ScoreData::is_legacy),
            None => false,
        },
    };

    let Nochoke {
        miss_limit,
        version,
        filter,
        ..
    } = args;

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&user_id, mode).await;
    let scores_fut = Context::osu_scores()
        .top(200, legacy_scores)
        .exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) => (user, scores),
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or scores");

            return Err(err);
        }
    };

    let version = version.unwrap_or_default();

    let mut entries = match process_scores(scores, miss_limit, version).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to process scores"));
        }
    };

    // Calculate bonus pp
    let actual_pp: f32 = entries
        .iter()
        .map(|entry| entry.original_score.pp)
        .zip(0..)
        .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

    let bonus_pp = user
        .statistics
        .as_ref()
        .expect("missing stats")
        .pp
        .to_native()
        - actual_pp;

    // Sort by unchoked pp
    entries.sort_unstable_by(|a, b| b.unchoked_pp().total_cmp(&a.unchoked_pp()));

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

    let rank = match Context::approx().rank(unchoked_pp, mode).await {
        Ok(rank) => Some(rank),
        Err(err) => {
            warn!(?err, "Failed to get rank pp");

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
        name = user.username.as_str(),
    );

    match filter {
        Some(NochokeFilter::OnlyChokes) => content.push_str(" (only chokes)"),
        Some(NochokeFilter::RemoveChokes) => content.push_str(" (removed chokes)"),
        None => {}
    }

    content.push(':');

    let pagination = NoChokePagination::builder()
        .user(user)
        .entries(entries.into_boxed_slice())
        .unchoked_pp(unchoked_pp)
        .rank(rank)
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
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

    pub fn unchoked_accuracy(&self) -> f32 {
        self.unchoked
            .as_ref()
            .map(|unchoked| match unchoked.max_statistics {
                Some(ref max_stats) => unchoked
                    .statistics
                    .accuracy(self.original_score.mode, max_stats),
                None => unchoked
                    .statistics
                    .legacy_accuracy(self.original_score.mode),
            })
            .unwrap_or(self.original_score.accuracy)
    }
}

pub struct Unchoked {
    pub grade: Grade,
    pub pp: f32,
    pub statistics: ScoreStatistics,
    pub max_statistics: Option<ScoreStatistics>,
}

impl Unchoked {
    fn new(if_fc: IfFc, mods: &GameMods, mode: GameMode) -> Self {
        let grade = calculate_grade(mode, mods, &if_fc.statistics, if_fc.max_statistics.as_ref());

        Self {
            grade,
            pp: if_fc.pp,
            statistics: if_fc.statistics,
            max_statistics: if_fc.max_statistics,
        }
    }
}

async fn process_scores(
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

    let mut maps = Context::osu_map().maps(&maps_id_checksum).await?;
    let miss_limit = miss_limit.unwrap_or(u32::MAX);

    for (i, score) in scores.into_iter().enumerate() {
        let Some(mut map) = maps.remove(&score.map_id) else {
            continue;
        };
        map = map.convert(score.mode);

        let attrs = Context::pp(&map)
            .lazer(score.set_on_lazer)
            .mode(score.mode)
            .mods(score.mods.clone())
            .performance()
            .await;

        let pp = score.pp.unwrap_or(0.0);

        let mut max_pp = 0.0;
        let mut stars = 0.0;
        let mut max_combo = 0;

        if let Some(attrs) = attrs {
            max_pp = attrs.pp() as f32;
            stars = attrs.stars() as f32;
            max_combo = attrs.max_combo();
        }

        if score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania && pp > 0.0 {
            max_pp = pp;
        }

        let score = ScoreSlim::new(score, pp);
        let too_many_misses = score.statistics.miss > miss_limit;

        let unchoked = match version {
            NochokeVersion::Unchoke if too_many_misses => None,
            // Skip unchoking because it has too many misses or because its a convert
            NochokeVersion::Unchoke => IfFc::new(&score, &map)
                .await
                .map(|if_fc| Unchoked::new(if_fc, &score.mods, score.mode)),
            NochokeVersion::Perfect if too_many_misses => None,
            NochokeVersion::Perfect => perfect_score(&score, &map).await,
        };

        let entry = NochokeEntry {
            original_idx: i,
            original_score: score,
            unchoked,
            map,
            max_pp,
            stars,
            max_combo,
        };

        entries.push(entry);
    }

    Ok(entries)
}

/// Returns `None` if the map is too suspicious.
async fn perfect_score(score: &ScoreSlim, map: &OsuMap) -> Option<Unchoked> {
    let total_hits = score.total_hits();
    let mut calc = Context::pp(map).mode(score.mode).mods(score.mods.clone());
    let attrs = calc.difficulty().await?;

    let stats = match attrs {
        DifficultyAttributes::Osu(attrs) if score.statistics.great != total_hits => {
            ScoreStatistics {
                great: total_hits,
                slider_tail_hit: attrs.n_sliders,
                small_tick_hit: attrs.n_sliders,
                large_tick_hit: attrs.n_large_ticks,
                ..Default::default()
            }
        }
        DifficultyAttributes::Taiko(_) if score.statistics.miss > 0 => ScoreStatistics {
            great: map.n_circles() as u32,
            ..Default::default()
        },
        DifficultyAttributes::Catch(attrs) if (100.0 - score.accuracy).abs() > f32::EPSILON => {
            ScoreStatistics {
                great: attrs.n_fruits,
                ok: attrs.n_droplets,
                meh: attrs.n_tiny_droplets,
                ..Default::default()
            }
        }
        DifficultyAttributes::Mania(_) if score.statistics.perfect != total_hits => {
            ScoreStatistics {
                perfect: total_hits,
                ..Default::default()
            }
        }
        _ => score.statistics.clone(), // Nothing to unchoke
    };

    let max_stats = score.set_on_lazer.then(|| stats.clone());

    let grade = calculate_grade(score.mode, &score.mods, &stats, max_stats.as_ref());

    let n_geki = match score.mode {
        GameMode::Osu | GameMode::Taiko | GameMode::Catch => 0,
        GameMode::Mania => stats.good,
    };

    let n_katu = match score.mode {
        GameMode::Osu | GameMode::Taiko => 0,
        GameMode::Catch => stats.small_tick_miss.max(stats.good),
        GameMode::Mania => stats.good,
    };

    let n100 = match score.mode {
        GameMode::Osu | GameMode::Taiko | GameMode::Mania => stats.ok,
        GameMode::Catch => stats.large_tick_hit.max(stats.ok),
    };

    let n50 = match score.mode {
        GameMode::Osu | GameMode::Mania => stats.meh,
        GameMode::Taiko => 0,
        GameMode::Catch => stats.small_tick_hit.max(stats.meh),
    };

    let large_tick_hits = match score.mode {
        GameMode::Osu => stats.large_tick_hit,
        GameMode::Taiko | GameMode::Catch | GameMode::Mania => 0,
    };

    let small_tick_hits = match score.mode {
        GameMode::Osu => stats.small_tick_hit,
        GameMode::Taiko | GameMode::Catch | GameMode::Mania => 0,
    };

    let slider_end_hits = match score.mode {
        GameMode::Osu => {
            if stats.slider_tail_hit > 0 {
                stats.slider_tail_hit
            } else {
                stats.small_tick_hit
            }
        }
        GameMode::Taiko | GameMode::Catch | GameMode::Mania => 0,
    };

    let pp = attrs
        .to_owned()
        .performance()
        .lazer(score.set_on_lazer)
        .mods(score.mods.clone())
        .clock_rate(score.mods.clock_rate().unwrap_or(1.0))
        .n_geki(n_geki)
        .n300(stats.perfect)
        .n_katu(n_katu)
        .n100(n100)
        .n50(n50)
        .misses(0)
        .large_tick_hits(large_tick_hits)
        .small_tick_hits(small_tick_hits)
        .slider_end_hits(slider_end_hits)
        .calculate()
        .pp() as f32;

    Some(Unchoked {
        grade,
        pp,
        statistics: stats,
        max_statistics: max_stats,
    })
}

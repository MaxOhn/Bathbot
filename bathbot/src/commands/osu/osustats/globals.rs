use std::{borrow::Cow, collections::BTreeMap, fmt::Write, ops::Not};

use bathbot_macros::command;
use bathbot_model::{
    OsuStatsParams, OsuStatsScore, OsuStatsScoresOrder, OsuStatsScoresRaw, ScoreSlim,
    command_fields::GameModeOption,
};
use bathbot_util::{
    CowUtils,
    constants::{GENERAL_ISSUE, OSUSTATS_API_ISSUE},
    matcher,
    osu::ModSelection,
};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameModIntermode, GameMode, Grade, OsuError, ScoreStatistics, Username};

use super::OsuStatsScores;
use crate::{
    Context,
    active::{ActiveMessages, impls::OsuStatsScoresPagination},
    commands::osu::{HasMods, ModsResult, user_not_found},
    core::commands::{CommandOrigin, prefix::Args},
    manager::{
        OsuMap,
        redis::osu::{UserArgs, UserArgsError},
    },
    util::ChannelExt,
};

const OSG_USAGE: &str = "[username] [mods] [acc=[number..]number] \
[rank=[integer..]integer] [sort=acc/combo/date/misses/pp/rank/score] \
[reverse=true/false]";

#[command]
#[desc("All scores of a player that are on a map's global leaderboard")]
#[help(
    "Show all scores of a player that are on a map's global leaderboard.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `rank`: single integer or two integers of the form `a..b` e.g. `rank=2..45`\n\
    - `sort`: `acc`, `combo`, `date` (default), `misses`, `pp`, `rank`, or `score`\n\
    - `reverse`: `true` or `false` (default)\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage(OSG_USAGE)]
#[examples(
    "badewanne3 -dt! acc=97.5..99.5 rank=42 sort=pp reverse=true",
    "vaxei sort=rank rank=1..5 +hdhr"
)]
#[aliases("osg", "osustatsglobal")]
#[group(Osu)]
async fn prefix_osustatsglobals(msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsScores::args(None, args) {
        Ok(args) => scores(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("All scores of a player that are on a map's global leaderboard")]
#[help(
    "Show all scores of a player that are on a mania map's global leaderboard.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `rank`: single integer or two integers of the form `a..b` e.g. `rank=2..45`\n\
    - `sort`: `acc`, `combo`, `date` (default), `misses`, `pp`, `rank`, or `score`\n\
    - `reverse`: `true` or `false` (default)\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage(OSG_USAGE)]
#[examples(
    "badewanne3 -dt! acc=97.5..99.5 rank=42 sort=pp reverse=true",
    "vaxei sort=rank rank=1..5 +hdhr"
)]
#[aliases("osgm", "osustatsglobalmania")]
#[group(Mania)]
async fn prefix_osustatsglobalsmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsScores::args(Some(GameModeOption::Mania), args) {
        Ok(args) => scores(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("All scores of a player that are on a map's global leaderboard")]
#[help(
    "Show all scores of a player that are on a taiko map's global leaderboard.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `rank`: single integer or two integers of the form `a..b` e.g. `rank=2..45`\n\
    - `sort`: `acc`, `combo`, `date` (default), `misses`, `pp`, `rank`, or `score`\n\
    - `reverse`: `true` or `false` (default)\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage(OSG_USAGE)]
#[examples(
    "badewanne3 -dt! acc=97.5..99.5 rank=42 sort=pp reverse=true",
    "vaxei sort=rank rank=1..5 +hdhr"
)]
#[aliases("osgt", "osustatsglobaltaiko")]
#[group(Taiko)]
async fn prefix_osustatsglobalstaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsScores::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => scores(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("All scores of a player that are on a map's global leaderboard")]
#[help(
    "Show all scores of a player that are on a ctb map's global leaderboard.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `rank`: single integer or two integers of the form `a..b` e.g. `rank=2..45`\n\
    - `sort`: `acc`, `combo`, `date` (default), `misses`, `pp`, `rank`, or `score`\n\
    - `reverse`: `true` or `false` (default)\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage(OSG_USAGE)]
#[examples(
    "badewanne3 -dt! acc=97.5..99.5 rank=42 sort=pp reverse=true",
    "vaxei sort=rank rank=1..5 +hdhr"
)]
#[aliases("osgc", "osustatsglobalctb", "osustatsglobalscatch")]
#[group(Catch)]
async fn prefix_osustatsglobalsctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match OsuStatsScores::args(Some(GameModeOption::Catch), args) {
        Ok(args) => scores(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

pub(super) async fn scores(orig: CommandOrigin<'_>, args: OsuStatsScores<'_>) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => return orig.error(OsuStatsScores::ERR_PARSE_MODS).await,
    };

    let (user_id, mode) = user_id_mode!(orig, args);
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    // Retrieve user
    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let params = args.into_params(user.username.as_str().into(), mode, mods);
    let scores_fut = Context::client().get_global_scores(&params);

    // Retrieve their top global scores
    let (scores, amount) = match scores_fut.await.map(OsuStatsScoresRaw::into_scores) {
        Ok(Ok(scores)) => (scores.scores, scores.count),
        Err(err) | Ok(Err(err)) => {
            let _ = orig.error(OSUSTATS_API_ISSUE).await;

            return Err(err.wrap_err("Failed to get global scores"));
        }
    };

    let entries = match process_scores(scores, mode).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to process scores"));
        }
    };

    let mut content = format!(
        "`Rank: {rank_min} - {rank_max}` • \
        `Acc: {acc_min}% - {acc_max}%` • \
        `Order: {order} {descending}`",
        acc_min = params.min_acc,
        acc_max = params.max_acc,
        rank_min = params.min_rank,
        rank_max = params.max_rank,
        order = params.order,
        descending = if params.descending { "Desc" } else { "Asc" },
    );

    if let Some(selection) = params.get_mods() {
        let _ = write!(
            content,
            " • `Mods: {}`",
            match selection {
                ModSelection::Exact(mods) => {
                    if mods.contains(GameModIntermode::Nightcore)
                        || mods.contains(GameModIntermode::Perfect)
                    {
                        let mut mods = mods.to_owned();

                        if mods.contains(GameModIntermode::Nightcore) {
                            mods.remove(GameModIntermode::DoubleTime);
                        }

                        if mods.contains(GameModIntermode::Perfect) {
                            mods.remove(GameModIntermode::SuddenDeath);
                        }

                        mods.to_string()
                    } else {
                        mods.to_string()
                    }
                }
                ModSelection::Exclude { mods, .. } => format!("Exclude {mods}"),
                ModSelection::Include(mods) => format!("Include {mods}"),
            },
        );
    }

    let pagination = OsuStatsScoresPagination::builder()
        .user(user)
        .entries(entries)
        .total(amount)
        .params(params)
        .content(content.into_boxed_str())
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

impl<'m> OsuStatsScores<'m> {
    const ERR_PARSE_ACC: &'static str = "Failed to parse `accuracy`.\n\
        Must be either decimal number \
        or two decimal numbers of the form `a..b` e.g. `97.5..98.5`.";
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
    If you want included mods, specify it e.g. as `+hrdt`.\n\
    If you want exact mods, specify it e.g. as `+hdhr!`.\n\
    And if you want to exclude mods, specify it e.g. as `-hdnf!`.";
    const ERR_PARSE_RANK: &'static str = "Failed to parse `rank`.\n\
        Must be either a positive integer \
        or two positive integers of the form `a..b` e.g. `2..45`.";
    const MAX_RANK: u32 = 100;
    const MIN_RANK: u32 = 1;

    fn into_params(
        self,
        username: Username,
        mode: GameMode,
        mods: Option<ModSelection>,
    ) -> OsuStatsParams {
        let mut params = OsuStatsParams::new(username);

        let order = self.sort.unwrap_or_default();
        let mut descending = self.reverse.is_none_or(bool::not);

        if order == OsuStatsScoresOrder::Rank {
            descending = !descending;
        }

        params
            .mode(mode)
            .page(1)
            .min_rank(self.min_rank.unwrap_or(Self::MIN_RANK) as usize)
            .max_rank(self.max_rank.unwrap_or(Self::MAX_RANK) as usize)
            .min_acc(self.min_acc.unwrap_or(0.0))
            .max_acc(self.max_acc.unwrap_or(100.0))
            .order(order)
            .descending(descending);

        if let Some(mods) = mods {
            params.mods(mods);
        }

        params
    }

    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut discord = None;
        let mut min_rank = None;
        let mut max_rank = None;
        let mut min_acc = None;
        let mut max_acc = None;
        let mut sort = None;
        let mut mods = None;
        let mut reverse = None;

        for arg in args.map(|arg| arg.cow_to_ascii_lowercase()) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "acc" | "accuracy" | "a" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                0.0
                            } else if let Ok(num) = bot.parse::<f32>() {
                                num.clamp(0.0, 100.0)
                            } else {
                                return Err(Self::ERR_PARSE_ACC.into());
                            };

                            let max = if top.is_empty() {
                                100.0
                            } else if let Ok(num) = top.parse::<f32>() {
                                num.clamp(0.0, 100.0)
                            } else {
                                return Err(Self::ERR_PARSE_ACC.into());
                            };

                            min_acc = Some(min.min(max));
                            max_acc = Some(min.max(max));
                        }
                        None => match value.parse() {
                            Ok(num) => min_acc = Some(num),
                            Err(_) => return Err(Self::ERR_PARSE_ACC.into()),
                        },
                    },
                    "rank" | "r" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                Self::MIN_RANK
                            } else if let Ok(num) = bot.parse::<u32>() {
                                num.clamp(Self::MIN_RANK, Self::MAX_RANK)
                            } else {
                                return Err(Self::ERR_PARSE_RANK.into());
                            };

                            let max = if top.is_empty() {
                                Self::MAX_RANK
                            } else if let Ok(num) = top.parse::<u32>() {
                                num.clamp(Self::MIN_RANK, Self::MAX_RANK)
                            } else {
                                return Err(Self::ERR_PARSE_RANK.into());
                            };

                            min_rank = Some(min.min(max));
                            max_rank = Some(min.max(max));
                        }
                        None => match value.parse() {
                            Ok(num) => max_rank = Some(num),
                            Err(_) => return Err(Self::ERR_PARSE_RANK.into()),
                        },
                    },
                    "sort" | "s" | "order" | "ordering" => match value {
                        "date" | "d" | "scoredate" => sort = Some(OsuStatsScoresOrder::Date),
                        "pp" => sort = Some(OsuStatsScoresOrder::Pp),
                        "rank" | "r" => sort = Some(OsuStatsScoresOrder::Rank),
                        "acc" | "accuracy" | "a" => sort = Some(OsuStatsScoresOrder::Acc),
                        "combo" | "c" => sort = Some(OsuStatsScoresOrder::Combo),
                        "score" | "s" => sort = Some(OsuStatsScoresOrder::Score),
                        "misses" | "miss" | "m" => sort = Some(OsuStatsScoresOrder::Misses),
                        _ => {
                            let content = "Failed to parse `sort`.\n\
                                Must be either `acc`, `combo`, `date`, `misses`, `pp`, `rank`, or `score`.";

                            return Err(content.into());
                        }
                    },
                    "reverse" => match value {
                        "true" | "t" | "1" => reverse = Some(true),
                        "false" | "f" | "0" => reverse = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `reverse`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    "mods" => match matcher::get_mods(value) {
                        Some(_) => mods = Some(format!("+{value}!").into()),
                        None => return Err(Self::ERR_PARSE_MODS.into()),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `acc`, `rank`, `sort`, or `reverse`."
                        );

                        return Err(content.into());
                    }
                }
            } else if matcher::get_mods(&arg).is_some() {
                mods = Some(arg);
            } else if let Some(id) = matcher::get_mention_user(&arg) {
                discord = Some(id);
            } else {
                name = Some(arg);
            }
        }

        Ok(Self {
            mode,
            name,
            sort,
            mods,
            min_rank,
            max_rank,
            min_acc,
            max_acc,
            reverse,
            discord,
        })
    }
}

pub struct OsuStatsEntry {
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub rank: u32,
    pub stars: f32,
    pub max_pp: f32,
    pub max_combo: u32,
}

async fn process_scores(
    scores: Vec<OsuStatsScore>,
    mode: GameMode,
) -> Result<BTreeMap<usize, OsuStatsEntry>> {
    let mut entries = BTreeMap::new();

    let maps_id_checksum = scores
        .iter()
        .map(|score| (score.map.map_id as i32, None))
        .collect();

    let mut maps = Context::osu_map().maps(&maps_id_checksum).await?;

    for (i, score) in scores.into_iter().enumerate() {
        let map_opt = maps.remove(&score.map.map_id);
        let Some(map) = map_opt else { continue };

        let mut calc = Context::pp(&map).mode(mode).mods(score.mods.clone());
        let attrs = calc.performance().await;

        let pp = match score.pp {
            Some(pp) => pp,
            None => match calc.score(&score).performance().await {
                Some(attrs) => attrs.pp() as f32,
                None => 0.0,
            },
        };

        let mut max_pp = 0.0;
        let mut stars = 0.0;
        let mut max_combo = 0;

        if let Some(attrs) = attrs {
            max_pp = attrs.pp() as f32;
            stars = attrs.stars() as f32;
            max_combo = attrs.max_combo();
        }

        if score.grade.eq_letter(Grade::X) && mode != GameMode::Mania && pp > 0.0 {
            max_pp = pp;
        }

        let rank = score.position;

        let score = ScoreSlim {
            accuracy: score.accuracy,
            ended_at: score.ended_at,
            grade: score.grade,
            max_combo: score.max_combo,
            mode,
            mods: score.mods,
            pp,
            score: score.score,
            classic_score: 0,
            score_id: 0,
            statistics: ScoreStatistics {
                perfect: score.count_geki,
                great: score.count300,
                good: score.count_katu,
                ok: score.count100,
                meh: score.count50,
                miss: score.count_miss,
                ..Default::default()
            },
            set_on_lazer: false, // FIXME: how does osustats handle lazer scores?
            is_legacy: true,
        };

        let entry = OsuStatsEntry {
            score,
            map,
            rank,
            max_pp,
            stars,
            max_combo,
        };

        entries.insert(i, entry);
    }

    Ok(entries)
}

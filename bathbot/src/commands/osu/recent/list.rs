use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    collections::{hash_map::Entry, HashMap},
    fmt::Write,
};

use bathbot_macros::command;
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::ModSelection,
    CowUtils, IntHasher,
};
use eyre::{Report, Result};
use rosu_pp::any::DifficultyAttributes;
use rosu_v2::{
    prelude::{GameMode, Grade, OsuError, Score},
    request::UserId,
};

use super::{RecentList, RecentListUnique};
use crate::{
    active::{impls::RecentListPagination, ActiveMessages},
    commands::{
        osu::{require_link, user_not_found, HasMods, ModsResult, ScoreOrder},
        GameModeOption, GradeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    manager::{redis::osu::UserArgs, Mods, OsuMap},
    util::{
        query::{IFilterCriteria, RegularCriteria, Searchable},
        ChannelExt,
    },
    Context,
};

#[command]
#[desc("Display a list of a user's most recent plays")]
#[help(
    "Display a list of a user's most recent plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("rl")]
#[group(Osu)]
async fn prefix_recentlist(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(None, args) {
        Ok(args) => list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a list of a user's most recent mania plays")]
#[help(
    "Display a list of a user's most recent mania plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("rlm")]
#[group(Mania)]
async fn prefix_recentlistmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Mania), args) {
        Ok(args) => list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a list of a user's most recent taiko plays")]
#[help(
    "Display a list of a user's most recent taiko plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("rlt")]
#[group(Taiko)]
async fn prefix_recentlisttaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a list of a user's most recent ctb plays")]
#[help(
    "Display a list of a user's most recent ctb plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlc", "recentlistcatch")]
#[group(Catch)]
async fn prefix_recentlistctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Catch), args) {
        Ok(args) => list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a list of a user's most recent passes")]
#[help(
    "Display a list of a user's most recent passes.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("rlp", "recentlistpasses", "rpl")]
#[group(Osu)]
async fn prefix_recentlistpass(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(None, args) {
        Ok(mut args) => {
            args.passes = Some(true);

            list(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a list of a user's most recent mania passes")]
#[help(
    "Display a list of a user's most recent mania passes.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("rlpm", "recentlistpassesmania", "rplm")]
#[group(Mania)]
async fn prefix_recentlistpassmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Mania), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            list(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a list of a user's most recent taiko plays")]
#[help(
    "Display a list of a user's most recent taiko plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("rlpt", "recentlistpassestaiko", "rplt")]
#[group(Taiko)]
async fn prefix_recentlistpasstaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Taiko), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            list(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a list of a user's most recent ctb plays")]
#[help(
    "Display a list of a user's most recent ctb plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "rlpc",
    "recentlistpasscatch",
    "recentlistpassesctb",
    "recentlistpassescatch",
    "rplc"
)]
#[group(Catch)]
async fn prefix_recentlistpassctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Catch), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            list(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

impl<'m> RecentList<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut discord = None;
        let mut grade = None;
        let mut passes = None;

        for arg in args.take(3).map(|arg| arg.cow_to_ascii_lowercase()) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "pass" | "p" | "passes" => match value {
                        "true" | "t" | "1" => passes = Some(true),
                        "false" | "f" | "0" => passes = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `pass`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    "fail" | "fails" | "f" => match value {
                        "true" | "t" | "1" => passes = Some(false),
                        "false" | "f" | "0" => passes = Some(true),
                        _ => {
                            let content =
                                "Failed to parse `fail`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    "grade" | "g" => match value.parse::<GradeOption>() {
                        Ok(grade_) => grade = Some(grade_),
                        Err(content) => return Err(content.into()),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `grade` or `pass`."
                        );

                        return Err(content.into());
                    }
                }
            } else if let Some(id) = matcher::get_mention_user(&arg) {
                discord = Some(id);
            } else {
                name = Some(arg);
            }
        }

        if passes.is_some() {
            grade = None;
        }

        Ok(Self {
            mode,
            name,
            query: None,
            grade,
            sort: None,
            passes,
            mods: None,
            unique: None,
            discord,
        })
    }
}

pub(super) async fn list(orig: CommandOrigin<'_>, args: RecentList<'_>) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(content).await;
        }
    };

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
        None => GameMode::Osu,
        Some(mode) => mode,
    };

    let legacy_scores = match config.score_data {
        Some(score_data) => score_data.is_legacy(),
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data.map(ScoreData::is_legacy))
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    let RecentList {
        query,
        grade,
        passes,
        ..
    } = &args;

    let grade = grade.map(Grade::from);

    // Retrieve the user and their recent scores
    let user_args = UserArgs::rosu_id(&user_id).await.mode(mode);

    let include_fails = match (grade, passes) {
        (Some(Grade::F), Some(true)) => return orig.error(":clown:").await,
        (_, Some(passes)) => !passes,
        (Some(Grade::F), _) | (None, None) => true,
        _ => false,
    };

    let scores_fut = Context::osu_scores()
        .recent(legacy_scores)
        .limit(100)
        .include_fails(include_fails)
        .exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) if scores.is_empty() => {
            let username = user.username();

            let content = format!(
                "No recent {}plays found for user `{username}`",
                match mode {
                    GameMode::Osu => "",
                    GameMode::Taiko => "taiko ",
                    GameMode::Catch => "ctb ",
                    GameMode::Mania => "mania ",
                },
            );

            return orig.error(content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or scores");

            return Err(err);
        }
    };

    let (entries, maps) = match process_scores(scores, &args, mode, mods.as_ref()).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to process scores"));
        }
    };

    let content = message_content(grade, mods.as_ref(), query.as_deref()).unwrap_or_default();

    let pagination = RecentListPagination::builder()
        .user(user)
        .entries(entries.into_boxed_slice())
        .maps(maps)
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

fn message_content(
    grade: Option<Grade>,
    mods: Option<&ModSelection>,
    query: Option<&str>,
) -> Option<String> {
    let mut content = String::new();

    if let Some(grade) = grade {
        let _ = write!(content, "`Grade: {grade}`");
    }

    if let Some(selection) = mods {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let (pre, mods) = match selection {
            ModSelection::Exact(mods) => ("", mods),
            ModSelection::Exclude(mods) => ("Exclude ", mods),
            ModSelection::Include(mods) => ("Include ", mods),
        };

        let _ = write!(content, "`Mods: {pre}{mods}`");
    }

    if let Some(query) = query {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let _ = write!(content, "`Query: {query}`");
    }

    (!content.is_empty()).then_some(content)
}

pub struct RecentListEntry {
    pub idx: usize,
    pub score: ScoreSlim,
    pub map_id: u32,
    // These three fields are likely duplicated across multiple
    // entries but they don't really hurt and provide convenience
    pub stars: f32,
    pub max_pp: f32,
    pub max_combo: u32,
}

async fn process_scores(
    scores: Vec<Score>,
    args: &RecentList<'_>,
    mode: GameMode,
    mods: Option<&ModSelection>,
) -> Result<(Vec<RecentListEntry>, HashMap<u32, OsuMap, IntHasher>)> {
    let RecentList {
        query,
        grade,
        passes,
        sort,
        unique,
        ..
    } = args;

    let filter_criteria = query.as_deref().map(RegularCriteria::create);
    let grade = grade.map(Grade::from);
    let mut entries = Vec::new();

    let score_filter = |score: &Score| {
        if filter_criteria.as_ref().is_some_and(|c| !score.matches(c)) {
            return false;
        }

        let grade_res = if let Some(grade) = grade {
            if let Grade::F = grade {
                !score.passed
            } else {
                score.grade.eq_letter(grade)
            }
        } else if let Some(passed) = passes {
            *passed == score.passed
        } else {
            true
        };

        if !grade_res {
            return false;
        }

        if mods.is_some_and(|selection| !selection.filter_score(score)) {
            return false;
        }

        true
    };

    let maps_id_checksum = scores
        .iter()
        .filter(|&score| score_filter(score))
        .filter_map(|score| score.map.as_ref())
        .map(|map| (map.map_id as i32, map.checksum.as_deref()))
        .collect();

    let mut maps = Context::osu_map().maps(&maps_id_checksum).await?;

    if mode != GameMode::Osu {
        maps.values_mut().for_each(|map| map.convert_mut(mode));
    }

    let mut attrs_map: HashMap<(u32, Mods), DifficultyAttributes> =
        HashMap::with_capacity(maps.len());

    let scores = scores
        .into_iter()
        .enumerate()
        .filter(|(_, score)| score_filter(score));

    for (idx, score) in scores {
        let Some(map) = maps.get(&score.map_id) else {
            continue;
        };

        let mods = Mods::from(&score.mods);
        let mut calc = Context::pp(map).mode(score.mode).mods(mods);

        let attrs = match attrs_map.entry((score.map_id, mods)) {
            Entry::Occupied(e) => {
                calc.attributes(e.get().to_owned());

                &*e.into_mut()
            }
            Entry::Vacant(e) => {
                let attrs = calc.difficulty().await;
                e.insert(attrs.to_owned());

                attrs
            }
        };

        let stars = attrs.stars() as f32;
        let max_combo = attrs.max_combo();

        let max_pp = match score
            .pp
            .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
        {
            Some(pp) => pp,
            None => calc.performance().await.pp() as f32,
        };

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        let map_id = score.map_id;
        let score = ScoreSlim::new(score, pp);

        let entry = RecentListEntry {
            idx,
            score,
            map_id,
            max_pp,
            stars,
            max_combo,
        };

        entries.push(entry);
    }

    match unique {
        None => {}
        Some(RecentListUnique::HighestPp) => {
            entries.sort_unstable_by(|a, b| {
                match a.map_id.cmp(&b.map_id) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }

                if a.score.mods != b.score.mods {
                    return Ordering::Less;
                }

                b.score.pp.total_cmp(&a.score.pp)
            });

            entries.dedup_by(|a, b| a.map_id.eq(&b.map_id) && a.score.mods.eq(&b.score.mods));
            entries.sort_unstable_by_key(|entry| Reverse(entry.score.ended_at));
        }
        Some(RecentListUnique::HighestScore) => {
            entries.sort_unstable_by(|a, b| {
                match a.map_id.cmp(&b.map_id) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }

                if a.score.mods != b.score.mods {
                    return Ordering::Less;
                }

                b.score.score.cmp(&a.score.score)
            });

            entries.dedup_by(|a, b| a.map_id.eq(&b.map_id) && a.score.mods.eq(&b.score.mods));
            entries.sort_unstable_by_key(|entry| Reverse(entry.score.ended_at));
        }
    }

    match sort {
        None => {}
        Some(ScoreOrder::Acc) => entries.sort_by(|a, b| {
            b.score
                .accuracy
                .partial_cmp(&a.score.accuracy)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Bpm) => entries.sort_by(|a, b| {
            let a_map = maps.get(&a.map_id).expect("missing map");
            let b_map = maps.get(&b.map_id).expect("missing map");

            b_map
                .bpm()
                .partial_cmp(&a_map.bpm())
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Combo) => entries.sort_by_key(|entry| Reverse(entry.score.max_combo)),
        Some(ScoreOrder::Date) => entries.sort_by_key(|entry| Reverse(entry.score.ended_at)),
        Some(ScoreOrder::Length) => {
            entries.sort_by(|a, b| {
                let a_map = maps.get(&a.map_id).expect("missing map");
                let b_map = maps.get(&b.map_id).expect("missing map");

                let a_len = a_map.seconds_drain() as f32 / a.score.mods.clock_rate().unwrap_or(1.0);
                let b_len = b_map.seconds_drain() as f32 / b.score.mods.clock_rate().unwrap_or(1.0);

                b_len
                    .partial_cmp(&a_len)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| {
                        if a_map.map_id() != b_map.map_id() {
                            Ordering::Equal
                        } else {
                            let a_is_fail = a.score.grade == Grade::F;
                            let b_is_fail = b.score.grade == Grade::F;
                            match (a_is_fail, b_is_fail) {
                                (true, true) => {
                                    let a_completion = a.score.statistics.total_hits(a.score.mode)
                                        as f32
                                        / a_map.n_objects() as f32;
                                    let b_completion = b.score.statistics.total_hits(b.score.mode)
                                        as f32
                                        / b_map.n_objects() as f32;

                                    b_completion
                                        .partial_cmp(&a_completion)
                                        .unwrap_or(Ordering::Equal)
                                }
                                (true, false) => Ordering::Greater,
                                (false, true) => Ordering::Less,
                                (false, false) => Ordering::Equal,
                            }
                        }
                    })
            });
        }
        Some(ScoreOrder::Misses) => entries.sort_by(|a, b| {
            b.score
                .statistics
                .count_miss
                .cmp(&a.score.statistics.count_miss)
                .then_with(|| {
                    let hits_a = a.score.total_hits();
                    let hits_b = b.score.total_hits();

                    let ratio_a = a.score.statistics.count_miss as f32 / hits_a as f32;
                    let ratio_b = b.score.statistics.count_miss as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
        }),
        Some(ScoreOrder::Pp) => entries.sort_by(|a, b| {
            b.score
                .pp
                .partial_cmp(&a.score.pp)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::RankedDate) => entries.sort_by_key(|entry| {
            let map = maps.get(&entry.map_id).expect("missing map");

            Reverse(map.ranked_date())
        }),
        Some(ScoreOrder::Score) => entries.sort_by_key(|entry| Reverse(entry.score.score)),
        Some(ScoreOrder::Stars) => {
            entries.sort_by(|a, b| b.stars.partial_cmp(&a.stars).unwrap_or(Ordering::Equal))
        }
    }

    Ok((entries, maps))
}

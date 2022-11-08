use std::{borrow::Cow, collections::HashMap, fmt::Write, sync::Arc};

use command_macros::command;
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, GameMods, Grade, OsuError};

use crate::{
    commands::{
        osu::{user_not_found, HasMods, ModsResult},
        GameModeOption, GradeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::osu::UserArgs,
    pagination::RecentListPagination,
    util::{
        constants::OSU_API_ISSUE,
        matcher,
        osu::ModSelection,
        query::{FilterCriteria, Searchable},
        ChannelExt, CowUtils,
    },
    Context,
};

use super::RecentList;

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
async fn prefix_recentlist(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(None, args) {
        Ok(args) => list(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
async fn prefix_recentlistmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Mania), args) {
        Ok(args) => list(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
async fn prefix_recentlisttaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => list(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
async fn prefix_recentlistctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentList::args(Some(GameModeOption::Catch), args) {
        Ok(args) => list(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
            passes,
            mods: None,
            discord,
        })
    }
}

pub(super) async fn list(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RecentList<'_>,
) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(&ctx, content).await;
        }
    };

    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    let RecentList {
        query,
        grade,
        passes,
        ..
    } = args;

    let grade = grade.map(Grade::from);

    // Retrieve the user and their recent scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);

    let include_fails = match (grade, passes) {
        (_, Some(passes)) => !passes,
        (Some(Grade::F), _) | (None, None) => true,
        _ => false,
    };

    let scores_fut = ctx
        .osu_scores()
        .recent()
        .limit(100)
        .include_fails(include_fails)
        .exec_with_user(user_args);

    let (user, mut scores) = match scores_fut.await {
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

            return orig.error(&ctx, content).await;
        }
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

    if let Some(grade) = grade {
        scores.retain(|score| score.grade.eq_letter(grade));
    } else if let Some(true) = passes {
        scores.retain(|score| score.grade != Grade::F);
    } else if let Some(false) = passes {
        scores.retain(|score| score.grade == Grade::F);
    }

    match mods {
        Some(ModSelection::Include(GameMods::NoMod)) => {
            scores.retain(|score| score.mods.is_empty())
        }
        Some(ModSelection::Include(mods)) => scores.retain(|score| score.mods.contains(mods)),
        Some(ModSelection::Exact(mods)) => scores.retain(|score| score.mods == mods),
        Some(ModSelection::Exclude(GameMods::NoMod)) => {
            scores.retain(|score| !score.mods.is_empty())
        }
        Some(ModSelection::Exclude(mods)) => {
            scores.retain(|score| score.mods.intersection(mods).is_empty())
        }
        None => {}
    }

    if let Some(query) = query.as_deref() {
        let criteria = FilterCriteria::new(query);
        scores.retain(|score| score.matches(&criteria));
    }

    let content = message_content(grade, mods, query);
    let maps = HashMap::default();
    let attr_map = HashMap::default();

    RecentListPagination::builder(user, scores, maps, attr_map)
        .content(content.unwrap_or_default())
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

fn message_content(
    grade: Option<Grade>,
    mods: Option<ModSelection>,
    query: Option<String>,
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

use std::{fmt::Write, sync::Arc};

use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    osu::ModSelection,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};

use super::{process_scores, ScoresOrder, UserScores};
use crate::{
    commands::osu::{require_link, user_not_found, HasMods, ModsResult},
    core::{commands::CommandOrigin, Context},
    manager::redis::{osu::UserArgs, RedisData},
    pagination::UserScoresPagination,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

pub async fn user_scores(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: UserScores,
) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let author_id = command.user_id()?;
    let config = ctx.user_config().with_osu_id(author_id).await?;

    let mode = args.mode.map(GameMode::from).or(config.mode);

    let user_id = {
        let orig = CommandOrigin::from(&mut command);

        match user_id!(ctx, orig, args).or_else(|| config.osu.map(UserId::Id)) {
            Some(user_id) => user_id,
            None => return require_link(&ctx, &orig).await,
        }
    };

    let user_fut = get_user(&ctx, &user_id, mode);

    let user = match user_fut.await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get user"));
        }
    };

    let ids = &[user.user_id() as i32];
    let scores_fut = ctx
        .osu_scores()
        .from_osu_ids(ids, mode, mods.as_ref(), None, None);

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let creator_id = match args.mapper {
        Some(ref mapper) => match UserArgs::username(&ctx, mapper).await {
            UserArgs::Args(args) => Some(args.user_id),
            UserArgs::User { user, .. } => Some(user.user_id),
            UserArgs::Err(OsuError::NotFound) => {
                let content = user_not_found(&ctx, UserId::Name(mapper.as_str().into())).await;
                command.error(&ctx, content).await?;

                return Ok(());
            }
            UserArgs::Err(err) => {
                let _ = command.error(&ctx, OSU_API_ISSUE).await;

                return Err(Report::new(err).wrap_err("Failed to get mapper"));
            }
        },
        None => None,
    };

    let sort = args.sort.unwrap_or_default();
    let content = msg_content(sort, mods.as_ref(), args.mapper.as_deref());
    process_scores(&mut scores, creator_id, sort, args.reverse);

    UserScoresPagination::builder(scores, user, mode, sort)
        .content(content)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}

async fn get_user(
    ctx: &Context,
    user_id: &UserId,
    mode: Option<GameMode>,
) -> Result<RedisData<User>, OsuError> {
    let mut args = UserArgs::rosu_id(ctx, user_id).await;

    if let Some(mode) = mode {
        args = args.mode(mode);
    }

    ctx.redis().osu_user(args).await
}

fn msg_content(sort: ScoresOrder, mods: Option<&ModSelection>, mapper: Option<&str>) -> String {
    let mut content = String::new();

    match mods {
        Some(ModSelection::Include(mods)) => {
            let _ = write!(content, "`Mods: Include {mods}`");
        }
        Some(ModSelection::Exclude(mods)) => {
            let _ = write!(content, "`Mods: Exclude {mods}`");
        }
        Some(ModSelection::Exact(mods)) => {
            let _ = write!(content, "`Mods: {mods}`");
        }
        None => {}
    }

    if let Some(mapper) = mapper {
        if !content.is_empty() {
            content.push_str(" • ");
        }

        let _ = write!(content, "`Mapper: {mapper}`");
    }

    if !content.is_empty() {
        content.push_str(" • ");
    }

    content.push_str("`Order: ");

    let order = match sort {
        ScoresOrder::Acc => "Accuracy",
        ScoresOrder::Ar => "AR",
        ScoresOrder::Bpm => "BPM",
        ScoresOrder::Combo => "Combo",
        ScoresOrder::Cs => "CS",
        ScoresOrder::Date => "Date",
        ScoresOrder::Hp => "HP",
        ScoresOrder::Length => "Length",
        ScoresOrder::Misses => "Miss count",
        ScoresOrder::Od => "OD",
        ScoresOrder::Pp => "PP",
        ScoresOrder::RankedDate => "Ranked date",
        ScoresOrder::Score => "Score",
        ScoresOrder::Stars => "Stars",
    };

    content.push_str(order);
    content.push('`');

    content
}

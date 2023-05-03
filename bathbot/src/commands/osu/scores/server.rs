use std::{borrow::Cow, fmt::Write, sync::Arc};

use bathbot_model::CountryCode;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    osu::ModSelection,
    CowUtils,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{Grade, OsuError},
    request::UserId,
};

use super::{
    criteria_to_content, get_mode, process_scores, separate_content, MapStatus, ServerScores,
};
use crate::{
    commands::osu::{user_not_found, HasMods, ModsResult},
    core::Context,
    manager::redis::osu::UserArgs,
    pagination::ServerScoresPagination,
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, ScoresCriteria},
        Authored, InteractionCommandExt,
    },
};

pub async fn server_scores(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: ServerScores,
) -> Result<()> {
    let Some(guild_id) = command.guild_id else {
        let content = "This command does not work in DMs";
        command.error(&ctx, content).await?;

        return Ok(());
    };

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

    let country_code = match args.country {
        Some(ref country) => match CountryCode::from_name(country) {
            Some(code) => Some(Cow::Owned(code.to_string())),
            None if country.len() == 2 => Some(country.cow_to_ascii_uppercase()),
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                command.error(&ctx, content).await?;

                return Ok(());
            }
        },
        None => None,
    };

    let guild_fut = ctx.cache.guild(guild_id);
    let members_fut = ctx.cache.members(guild_id);
    let mode_fut = get_mode(&ctx, args.mode, command.user_id()?);

    let (guild_res, members_res, mode_res) = tokio::join!(guild_fut, members_fut, mode_fut);

    let guild_icon = guild_res
        .ok()
        .flatten()
        .and_then(|guild| Some((guild.id, *guild.icon.as_ref()?)));

    let members: Vec<_> = match members_res {
        Ok(members) => members.into_iter().map(|id| id as i64).collect(),
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = mode_res.unwrap_or_else(|err| {
        warn!(?err);

        None
    });

    let grade = args.grade.map(Grade::from);

    let scores_fut = ctx.osu_scores().from_discord_ids(
        &members,
        mode,
        mods.as_ref(),
        country_code.as_deref(),
        None,
        grade,
    );

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
    let criteria = args
        .query
        .as_deref()
        .map(FilterCriteria::<ScoresCriteria<'_>>::new);

    let content = msg_content(
        mods.as_ref(),
        args.mapper.as_deref(),
        country_code.as_deref(),
        args.status,
        grade,
        criteria.as_ref(),
    );

    process_scores(
        &mut scores,
        creator_id,
        sort,
        args.status,
        criteria.as_ref(),
        args.per_user,
        args.reverse,
    );

    ServerScoresPagination::builder(scores, mode, sort, guild_icon)
        .content(content)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}

fn msg_content(
    mods: Option<&ModSelection>,
    mapper: Option<&str>,
    country: Option<&str>,
    status: Option<MapStatus>,
    grade: Option<Grade>,
    criteria: Option<&FilterCriteria<ScoresCriteria<'_>>>,
) -> String {
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
        separate_content(&mut content);
        let _ = write!(content, "`Mapper: {mapper}`");
    }

    if let Some(country) = country {
        separate_content(&mut content);
        let _ = write!(content, "`Country: {country}`");
    }

    if let Some(status) = status {
        separate_content(&mut content);

        let status = match status {
            MapStatus::Ranked => "Ranked",
            MapStatus::Loved => "Loved",
            MapStatus::Approved => "Approved",
        };

        let _ = write!(content, "`Status: {status}`");
    }

    if let Some(grade) = grade {
        separate_content(&mut content);
        let _ = write!(content, "`Grade: {grade:?}`");
    }

    if let Some(criteria) = criteria {
        criteria_to_content(&mut content, criteria);
    }

    content
}

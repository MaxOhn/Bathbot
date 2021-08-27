use crate::{
    database::UserConfig,
    embeds::{EmbedData, LeaderboardEmbed},
    pagination::{LeaderboardPagination, Pagination},
    util::{
        constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
        matcher,
        osu::ModSelection,
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::id::UserId;

pub(super) async fn _recentleaderboard(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentLeaderboardArgs,
    national: bool,
) -> BotResult<()> {
    let RecentLeaderboardArgs {
        config,
        name,
        index,
        mods,
    } = args;

    let author_name = config.name;

    let name = match name.as_ref().or_else(|| author_name.as_ref()) {
        Some(name) => name.as_str(),
        None => return super::require_link(&ctx, &data).await,
    };

    let limit = index.map_or(1, |n| n + (n == 0) as usize);

    if limit > 50 {
        let content = "Recent history goes only 50 scores back.";

        return data.error(&ctx, content).await;
    }

    let mode = config.mode.unwrap_or(GameMode::STD);

    // Retrieve the recent scores
    let scores_fut = ctx
        .osu()
        .user_scores(name)
        .recent()
        .include_fails(true)
        .mode(mode)
        .limit(limit);

    let (map, mapset, user) = match scores_fut.await {
        Ok(scores) if scores.len() < limit => {
            let content = format!(
                "There are only {} many scores in `{}`'{} recent history.",
                scores.len(),
                name,
                if name.ends_with('s') { "" } else { "s" }
            );

            return data.error(&ctx, content).await;
        }
        Ok(mut scores) => match scores.pop() {
            Some(mut score) => {
                let map = score.map.take().unwrap();
                let mapset = score.mapset.take().unwrap();
                let user = score.user.take().unwrap();

                (map, mapset, user)
            }
            None => {
                let content = format!(
                    "No recent {}plays found for user `{}`",
                    match mode {
                        GameMode::STD => "",
                        GameMode::TKO => "taiko ",
                        GameMode::CTB => "ctb ",
                        GameMode::MNA => "mania ",
                    },
                    name
                );

                return data.error(&ctx, content).await;
            }
        },
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Retrieve the map's leaderboard
    let scores_fut = ctx.clients.custom.get_leaderboard(
        map.map_id,
        national,
        match mods {
            Some(ModSelection::Exclude(_)) | None => None,
            Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
        },
        mode,
    );

    let scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, OSU_WEB_ISSUE).await;

            return Err(why.into());
        }
    };

    let amount = scores.len();

    // Accumulate all necessary data
    let first_place_icon = scores
        .first()
        .map(|_| format!("{}{}", AVATAR_URL, user.user_id));

    let data_fut = LeaderboardEmbed::new(
        author_name.as_deref(),
        &map,
        Some(&mapset),
        (!scores.is_empty()).then(|| scores.iter().take(10)),
        &first_place_icon,
        0,
    );

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sending the embed
    let content = format!(
        "I found {} scores with the specified mods on the map's leaderboard",
        amount
    );

    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // Store map in DB
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        unwind_error!(warn, why, "Error while storing recent lb map in DB: {}");
    }

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = LeaderboardPagination::new(
        response,
        map,
        Some(mapset),
        scores,
        author_name,
        first_place_icon,
    );

    gb.execute(&ctx).await;
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (recentleaderboard): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rblb")]
pub async fn recentbelgianleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(recent_args.config.mode(GameMode::STD));
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, true).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a mania map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rmblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rmblb")]
pub async fn recentmaniabelgianleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::MNA);
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, true).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a taiko map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rtblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rtblb")]
pub async fn recenttaikobelgianleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::TKO);
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, true).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Belgian leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the belgian leaderboard of a ctb map that a user recently played.\n\
     Mods can be specified.\n\
     To get a previous recent map, you can add a number right after the command,\n\
     e.g. `rcblb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rcblb")]
pub async fn recentctbbelgianleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::CTB);
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, true).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rlb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rlb", "rglb", "recentgloballeaderboard")]
pub async fn recentleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(recent_args.config.mode(GameMode::STD));
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, false).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a mania map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rmlb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rmlb", "rmglb", "recentmaniagloballeaderboard")]
pub async fn recentmanialeaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::MNA);
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, false).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a taiko map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rtlb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rtlb", "rtglb", "recenttaikogloballeaderboard")]
pub async fn recenttaikoleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::TKO);
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, false).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Global leaderboard of a map that a user recently played")]
#[long_desc(
    "Display the global leaderboard of a ctb map that a user recently played.\n\
    Mods can be specified.\n\
    To get a previous recent map, you can add a number right after the command,\n\
    e.g. `rclb42 badewanne3` to get the 42nd most recent map."
)]
#[usage("[username] [+mods]")]
#[example("badewanne3 +hdhr")]
#[aliases("rclb", "rcglb", "recentctbgloballeaderboard")]
pub async fn recentctbleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentLeaderboardArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::CTB);
                    let data = CommandData::Message { msg, args, num };

                    _recentleaderboard(ctx, data, recent_args, false).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

pub(super) struct RecentLeaderboardArgs {
    pub config: UserConfig,
    pub name: Option<Name>,
    pub index: Option<usize>,
    pub mods: Option<ModSelection>,
}

impl RecentLeaderboardArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        index: Option<usize>,
    ) -> BotResult<Result<Self, &'static str>> {
        let config = ctx.user_config(author_id).await?;
        let mut name = None;
        let mut mods = None;

        for arg in args {
            if let Some(mods_) = matcher::get_mods(arg) {
                mods.replace(mods_);
            } else {
                match Args::check_user_mention(ctx, arg).await? {
                    Ok(name_) => name = Some(name_),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        let args = Self {
            config,
            name,
            index,
            mods,
        };

        Ok(Ok(args))
    }
}

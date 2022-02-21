use std::sync::Arc;

use rosu_v2::prelude::{Beatmap, GameMode, OsuError, Score, Username};
use twilight_model::application::interaction::{
    application_command::{CommandDataOption, CommandOptionValue},
    ApplicationCommand,
};

use crate::{
    commands::{
        osu::{get_user_and_scores, unchoke_pp, ScoreArgs, UserArgs},
        parse_discord, parse_mode_option, DoubleResultCow,
    },
    core::{commands::CommandData, Context},
    embeds::{EmbedData, FixScoreEmbed},
    error::Error,
    tracking::process_osu_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, INDEX, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        InteractionExt, MessageExt,
    },
    BotResult,
};

pub(super) async fn _fix(ctx: Arc<Context>, data: CommandData<'_>, args: FixArgs) -> BotResult<()> {
    let FixArgs { mode, name, index } = args;

    if mode == GameMode::MNA {
        return data.error(&ctx, "Can't fix mania scores \\:(").await;
    }

    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their recent scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::recent(100).include_fails(true);

    let (mut user, scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{name}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
            );

            return data.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    let num = index.unwrap_or(1).saturating_sub(1);
    let scores_len = scores.len();

    let (mut score, map, user, mut scores) = match scores.into_iter().nth(num) {
        Some(mut score) => {
            let mapset_fut = ctx
                .psql()
                .get_beatmapset(score.map.as_ref().unwrap().mapset_id);

            let best_fut = ctx
                .osu()
                .user_scores(score.user_id)
                .mode(mode)
                .limit(100)
                .best();

            let user_fut = ctx.osu().user(score.user_id).mode(mode);
            let score_fut = super::prepare_score(&ctx, &mut score);

            match tokio::join!(mapset_fut, score_fut, user_fut, best_fut) {
                (_, Err(why), ..) | (.., Err(why), _) | (.., Err(why)) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
                (Ok(mapset), Ok(_), Ok(user), Ok(best)) => {
                    let mut map = score.map.take().unwrap();
                    map.mapset = Some(mapset);

                    (score, map, user, best)
                }
                (Err(_), Ok(_), Ok(user), Ok(best)) => {
                    let mut map = score.map.take().unwrap();

                    let mapset = match ctx.osu().beatmapset(map.mapset_id).await {
                        Ok(mapset) => mapset,
                        Err(why) => {
                            let _ = data.error(&ctx, OSU_API_ISSUE).await;

                            return Err(why.into());
                        }
                    };

                    map.mapset = Some(mapset);

                    (score, map, user, best)
                }
            }
        }
        None => {
            let content = format!(
                "There {verb} only {num} score{plural} in `{name}`'{genitive} recent history.",
                verb = if scores_len != 1 { "are" } else { "is" },
                num = scores_len,
                plural = if scores_len != 1 { "s" } else { "" },
                name = name,
                genitive = if name.ends_with('s') { "" } else { "s" }
            );

            return data.error(&ctx, content).await;
        }
    };

    let unchoked_pp = if score.pp.is_some() && !needs_unchoking(&score, &map) {
        None
    } else {
        match unchoke_pp(&mut score, &map).await {
            Ok(pp) => pp,
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        }
    };

    // Process tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let gb = ctx.map_garbage_collector(&map);

    let embed_data = FixScoreEmbed::new(user, map, Some((score, scores)), unchoked_pp, None);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    // Set map on garbage collection list if unranked
    gb.execute(&ctx).await;

    Ok(())
}

fn needs_unchoking(score: &Score, map: &Beatmap) -> bool {
    match map.mode {
        GameMode::STD => {
            score.statistics.count_miss > 0
                || score.max_combo < map.max_combo.map_or(0, |c| c.saturating_sub(5))
        }
        GameMode::TKO => score.statistics.count_miss > 0,
        GameMode::CTB => score.max_combo != map.max_combo.unwrap_or(0),
        GameMode::MNA => panic!("can not unchoke mania scores"),
    }
}

pub(super) struct FixArgs {
    mode: GameMode,
    name: Option<Username>,
    index: Option<usize>,
}

impl FixArgs {
    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut name = None;
        let mut index = None;

        for option in options {
            match option.value {
                CommandOptionValue::Integer(value) => match option.name.as_str() {
                    INDEX => index = Some(value.max(1).min(100) as usize),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => name = Some(value.into()),
                    MODE => config.mode = parse_mode_option(&value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => name = Some(osu.into_username()),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let mode = config.mode.unwrap_or_default();
        let name = name.or_else(|| config.into_username());

        Ok(Ok(Self { mode, name, index }))
    }
}

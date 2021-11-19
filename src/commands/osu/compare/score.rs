use crate::{
    commands::{check_user_mention, parse_discord, DoubleResultCow},
    database::UserConfig,
    embeds::{CompareEmbed, EmbedData, NoScoresEmbed},
    error::Error,
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MAP, MAP_PARSE_FAIL, MODS, MODS_PARSE_FAIL, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher,
        osu::{map_id_from_history, map_id_from_msg, MapIdType, ModSelection},
        InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use eyre::Report;
use rosu_v2::prelude::{GameMods, OsuError, RankStatus::Ranked, Username};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    channel::message::MessageType,
    id::UserId,
};

#[command]
#[short_desc("Compare a player's score on a map")]
#[long_desc(
    "Display a user's top score on a given map. \n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified."
)]
#[usage("[username] [map url / map id] [+mods]")]
#[example(
    "badewanne3",
    "badewanne3 2240404 +hdhr",
    "badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
#[aliases("c")]
async fn compare(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ScoreArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut score_args)) => {
                    let reply = msg
                        .referenced_message
                        .as_ref()
                        .filter(|_| msg.kind == MessageType::Reply);

                    if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                        score_args.map = Some(id);
                    }

                    _compare(ctx, CommandData::Message { msg, args, num }, score_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_compare(ctx, *command).await,
    }
}

pub(super) async fn _compare(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: ScoreArgs,
) -> BotResult<()> {
    let ScoreArgs { config, mods, map } = args;
    let embeds_maximized = config.embeds_maximized();

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let map_id = if let Some(id) = map {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(data.channel_id()).await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };

        match map_id_from_history(&msgs) {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                return data.error(&ctx, content).await;
            }
        }
    };

    let map_id = match map_id {
        MapIdType::Map(id) => id,
        MapIdType::Set(_) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return data.error(&ctx, content).await;
        }
    };

    let arg_mods = match mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) => Some(mods),
    };

    let score_fut = ctx.osu().beatmap_user_score(map_id, name.as_str());

    let score_result = match arg_mods {
        None => score_fut.await,
        Some(mods) => score_fut.mods(mods).await,
    };

    // Retrieve user's score on the map
    let mut score = match score_result {
        Ok(mut score) => match super::prepare_score(&ctx, &mut score.score).await {
            Ok(_) => score,
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
        Err(OsuError::NotFound) => return no_scores(ctx, &data, name, map_id, arg_mods).await,
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let map = score.score.map.as_ref().unwrap();
    let mapset_id = map.mapset_id;

    // First try to just get the mapset from the DB
    let mapset_fut = ctx.psql().get_beatmapset(mapset_id);
    let user_fut = ctx.osu().user(score.score.user_id).mode(score.score.mode);

    let scores_fut = async {
        if map.status == Ranked {
            let fut = ctx
                .osu()
                .user_scores(score.score.user_id)
                .best()
                .limit(100)
                .mode(score.score.mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let (user, scores_opt) = match tokio::join!(mapset_fut, user_fut, scores_fut) {
        (_, Err(why), _) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        (Ok(mapset), Ok(user), scores_opt) => {
            score.score.mapset.replace(mapset);

            (user, scores_opt)
        }
        (Err(_), Ok(user), scores_opt) => {
            let mapset = match ctx.osu().beatmapset(mapset_id).await {
                Ok(mapset) => mapset,
                Err(why) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            };

            score.score.mapset.replace(mapset.into());

            (user, scores_opt)
        }
    };

    let mut best = match scores_opt {
        Some(Ok(scores)) => Some(scores),
        None => None,
        Some(Err(why)) => {
            let report = Report::new(why).wrap_err("failed to get top scores");
            warn!("{:?}", report);

            None
        }
    };

    // Accumulate all necessary data
    let mode = score.score.mode;

    let embed_data =
        match CompareEmbed::new(&user, best.as_deref(), score, arg_mods.is_some()).await {
            Ok(data) => data,
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };

    // Only maximize if config allows it
    if embeds_maximized {
        let builder = embed_data.as_builder().build().into();
        let response = data.create_message(&ctx, builder).await?.model().await?;

        ctx.store_msg(response.id);

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            process_tracking(&ctx, mode, scores, Some(&user)).await;
        }

        // Wait for minimizing
        tokio::spawn(async move {
            sleep(Duration::from_secs(45)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let builder = embed_data.into_builder().build().into();

            if let Err(why) = response.update_message(&ctx, builder).await {
                let report = Report::new(why).wrap_err("failed to minimize message");
                warn!("{:?}", report);
            }
        });
    } else {
        let builder = embed_data.into_builder().build().into();
        data.create_message(&ctx, builder).await?;

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            process_tracking(&ctx, mode, scores, Some(&user)).await;
        }
    }

    Ok(())
}

async fn no_scores(
    ctx: Arc<Context>,
    data: &CommandData<'_>,
    name: Username,
    map_id: u32,
    mods: Option<GameMods>,
) -> BotResult<()> {
    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!("There is no map with id {}", map_id);

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    let user = match super::request_user(&ctx, name.as_str(), map.mode).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{}`", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Sending the embed
    let embed = NoScoresEmbed::new(user, map, mods).into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

pub(super) struct ScoreArgs {
    config: UserConfig,
    mods: Option<ModSelection>,
    map: Option<MapIdType>,
}

impl ScoreArgs {
    async fn args(ctx: &Context, args: &mut Args<'_>, author_id: UserId) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut map = None;
        let mut mods = None;

        for arg in args.take(3) {
            if let Some(mods_) = matcher::get_mods(arg) {
                mods.replace(mods_);
            } else if let Some(id) =
                matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
            {
                map = Some(id);
            } else {
                match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        Ok(Ok(Self { config, map, mods }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut map = None;
        let mut mods = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MAP => match matcher::get_osu_map_id(&value)
                        .or_else(|| matcher::get_osu_mapset_id(&value))
                    {
                        Some(id) => map = Some(id),
                        None => return Ok(Err(MAP_PARSE_FAIL.into())),
                    },
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => match value.parse() {
                            Ok(mods_) => mods = Some(ModSelection::Exact(mods_)),
                            Err(_) => return Ok(Err(MODS_PARSE_FAIL.into())),
                        },
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => config.osu = Some(osu),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        Ok(Ok(ScoreArgs { config, mods, map }))
    }
}

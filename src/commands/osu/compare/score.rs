use crate::{
    database::UserConfig,
    embeds::{CompareEmbed, EmbedData, NoScoresEmbed},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher,
        osu::{
            cached_message_extract, map_id_from_history, map_id_from_msg, MapIdType, ModSelection,
        },
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use rosu_v2::prelude::{GameMods, OsuError, RankStatus::Ranked};
use std::{borrow::Cow, sync::Arc};
use tokio::time::{sleep, Duration};
use twilight_model::{
    application::interaction::application_command::CommandDataOption,
    channel::message::MessageType, id::UserId,
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
                        .filter(|msg| msg.kind == MessageType::Reply);

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

    let name = match config.name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let channel_id = data.channel_id();

    let map_id_opt = map.or_else(|| {
        let result = ctx
            .cache
            .message_extract(channel_id, cached_message_extract);

        if result.is_some() {
            ctx.stats.message_retrievals.cached.inc();
        }

        result
    });

    let map_id = if let Some(id) = map_id_opt {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(channel_id).await {
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
            unwind_error!(warn, why, "Failed to get top scores for compare: {}");

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
    if config.recent_embed_maximize {
        let builder = embed_data.as_builder().build().into();
        let response = data.create_message(&ctx, builder).await?.model().await?;

        ctx.store_msg(response.id);

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
                unwind_error!(warn, why, "Error while storing best maps in DB: {}");
            }

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
                unwind_error!(warn, why, "Error minimizing compare msg: {}");
            }
        });
    } else {
        let builder = embed_data.into_builder().build().into();
        data.create_message(&ctx, builder).await?;

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
                unwind_error!(warn, why, "Error while storing best maps in DB: {}");
            }

            process_tracking(&ctx, mode, scores, Some(&user)).await;
        }
    }

    Ok(())
}

async fn no_scores(
    ctx: Arc<Context>,
    data: &CommandData<'_>,
    name: Name,
    map_id: u32,
    mods: Option<GameMods>,
) -> BotResult<()> {
    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                if let Err(why) = ctx.psql().insert_beatmap(&map).await {
                    unwind_error!(warn, why, "Error while inserting compare map: {}");
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

    let user = match super::request_user(&ctx, name.as_str(), Some(map.mode)).await {
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
    const ERR_PARSE_MAP: &'static str = "Failed to parse map url.\n\
        Be sure you specify a valid map id or url to a map.";

    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        Be sure it's a valid mod abbreviation e.g. `hdhr`.";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, &'static str>> {
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
                match Args::check_user_mention(ctx, arg).await? {
                    Ok(name) => config.name = Some(name),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        Ok(Ok(Self { config, map, mods }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut map = None;
        let mut mods = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "name" => config.name = Some(value.into()),
                    "discord" => config.name = parse_discord_option!(ctx, value, "compare score"),
                    "map" => match matcher::get_osu_map_id(&value)
                        .or_else(|| matcher::get_osu_mapset_id(&value))
                    {
                        Some(id) => map = Some(id),
                        None => return Ok(Err(Self::ERR_PARSE_MAP.into())),
                    },
                    "mods" => match value.parse() {
                        Ok(mods_) => mods = Some(ModSelection::Include(mods_)),
                        Err(_) => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
                    _ => bail_cmd_option!("compare score", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("compare score", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("compare score", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("compare score", subcommand, name)
                }
            }
        }

        Ok(Ok(ScoreArgs { config, mods, map }))
    }
}

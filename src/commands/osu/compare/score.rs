use std::sync::Arc;

use eyre::Report;
use rosu_v2::prelude::{
    GameMode, GameMods, OsuError,
    RankStatus::{Approved, Loved, Ranked},
    Score,
};
use tokio::time::{sleep, Duration};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    channel::message::MessageType,
    id::UserId,
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_beatmap_user_score, get_user, UserArgs},
        parse_discord, DoubleResultCow,
    },
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
                        score_args.id = Some(MapOrScore::Map(id));
                    } else if let Some((mode, id)) =
                        reply.and_then(|msg| matcher::get_osu_score_id(&msg.content))
                    {
                        score_args.id = Some(MapOrScore::Score { id, mode });
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
    let ScoreArgs { config, mods, id } = args;

    let embeds_maximized = match (config.embeds_maximized, data.guild_id()) {
        (Some(embeds_maximized), _) => embeds_maximized,
        (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
        (None, None) => true,
    };

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let (score, global_idx) = match id {
        Some(MapOrScore::Map(MapIdType::Map(id))) => {
            match retrieve_data(&ctx, &data, name.as_str(), id, mods).await {
                ScoreResult::Score { score, global_idx } => (score, global_idx),
                ScoreResult::Done => return Ok(()),
                ScoreResult::Error(err) => return Err(err),
            }
        }
        Some(MapOrScore::Map(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return data.error(&ctx, content).await;
        }
        Some(MapOrScore::Score { id, mode }) => {
            let mut score = match ctx.osu().score(id, mode).await {
                Ok(score) => score,
                Err(err) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
                }
            };

            match ctx.osu().user(score.user_id).mode(mode).await {
                Ok(user) => score.user = Some(user.into()),
                Err(err) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
                }
            }

            let map = score.map.as_ref().unwrap();

            let global_idx = if matches!(map.status, Ranked | Loved | Approved) {
                match ctx.osu().beatmap_scores(map.map_id).mode(mode).await {
                    Ok(scores) => scores.iter().position(|s| s == &score),
                    Err(err) => {
                        let report = Report::new(err).wrap_err("failed to get global scores");
                        warn!("{:?}", report);

                        None
                    }
                }
            } else {
                None
            };

            (score, global_idx.map_or(usize::MAX, |idx| idx + 1))
        }
        None => {
            let msgs = match ctx.retrieve_channel_history(data.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            };

            match map_id_from_history(&msgs) {
                Some(MapIdType::Map(id)) => {
                    match retrieve_data(&ctx, &data, name.as_str(), id, mods).await {
                        ScoreResult::Score { score, global_idx } => (score, global_idx),
                        ScoreResult::Done => return Ok(()),
                        ScoreResult::Error(err) => return Err(err),
                    }
                }
                Some(MapIdType::Set(_)) => {
                    let content = "I found a mapset in the channel history but I need a map. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return data.error(&ctx, content).await;
                }
                None => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return data.error(&ctx, content).await;
                }
            }
        }
    };

    let mode = score.mode;

    let mut best = if score.map.as_ref().unwrap().status == Ranked {
        let fut = ctx
            .osu()
            .user_scores(score.user_id)
            .best()
            .limit(100)
            .mode(mode);

        match fut.await {
            Ok(scores) => Some(scores),
            Err(why) => {
                let report = Report::new(why).wrap_err("failed to get top scores");
                warn!("{:?}", report);

                None
            }
        }
    } else {
        None
    };

    // Accumulate all necessary data
    let embed_data =
        match CompareEmbed::new(best.as_deref(), score, mods.is_some(), global_idx).await {
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
            process_tracking(&ctx, scores, None).await;
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
            process_tracking(&ctx, scores, None).await;
        }
    }

    Ok(())
}

#[allow(clippy::large_enum_variant)]
enum ScoreResult {
    Score { score: Score, global_idx: usize },
    Done,
    Error(Error),
}

async fn retrieve_data(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    map_id: u32,
    mods: Option<ModSelection>,
) -> ScoreResult {
    let mods = match mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods) | ModSelection::Include(mods)) => Some(mods),
    };

    let user_args = UserArgs::new(name, GameMode::STD);
    let score_fut = get_beatmap_user_score(ctx.osu(), map_id, &user_args, mods);

    // Retrieve user's score on the map
    let (mut score, global_idx) = match score_fut.await {
        Ok(mut score) => match super::prepare_score(ctx, &mut score.score).await {
            Ok(_) => (score.score, score.pos),
            Err(why) => {
                let _ = data.error(ctx, OSU_API_ISSUE).await;

                return ScoreResult::Error(why.into());
            }
        },
        Err(OsuError::NotFound) => {
            return match no_scores(ctx, data, name, map_id, mods).await {
                Ok(_) => ScoreResult::Done,
                Err(err) => ScoreResult::Error(err),
            }
        }
        Err(why) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return ScoreResult::Error(why.into());
        }
    };

    let mapset_id = score.map.as_ref().unwrap().mapset_id;

    // First try to just get the mapset from the DB
    let mapset_fut = ctx.psql().get_beatmapset(mapset_id);
    let user_fut = ctx.osu().user(score.user_id).mode(score.mode);

    let user = match tokio::join!(mapset_fut, user_fut) {
        (_, Err(why)) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return ScoreResult::Error(why.into());
        }
        (Ok(mapset), Ok(user)) => {
            score.mapset = Some(mapset);

            user
        }
        (Err(_), Ok(user)) => {
            let mapset = match ctx.osu().beatmapset(mapset_id).await {
                Ok(mapset) => mapset,
                Err(why) => {
                    let _ = data.error(ctx, OSU_API_ISSUE).await;

                    return ScoreResult::Error(why.into());
                }
            };

            score.mapset = Some(mapset.into());

            user
        }
    };

    score.user = Some(user.into());

    ScoreResult::Score { score, global_idx }
}

async fn no_scores(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
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

                return data.error(ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    let user_args = UserArgs::new(name, map.mode);

    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{}`", name);

            return data.error(ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Sending the embed
    let embed = NoScoresEmbed::new(user, map, mods).into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    data.create_message(ctx, builder).await?;

    Ok(())
}

enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

pub(super) struct ScoreArgs {
    config: UserConfig,
    mods: Option<ModSelection>,
    id: Option<MapOrScore>,
}

impl ScoreArgs {
    async fn args(ctx: &Context, args: &mut Args<'_>, author_id: UserId) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut id = None;
        let mut mods = None;

        for arg in args.take(3) {
            if let Some(mods_) = matcher::get_mods(arg) {
                mods.replace(mods_);
            } else if let Some(id_) =
                matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
            {
                id = Some(MapOrScore::Map(id_));
            } else if let Some((mode, id_)) = matcher::get_osu_score_id(arg) {
                id = Some(MapOrScore::Score { mode, id: id_ })
            } else {
                match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        Ok(Ok(Self { config, id, mods }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut id = None;
        let mut mods = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MAP => match matcher::get_osu_map_id(&value)
                        .or_else(|| matcher::get_osu_mapset_id(&value))
                    {
                        Some(id_) => id = Some(MapOrScore::Map(id_)),
                        None => match matcher::get_osu_score_id(&value) {
                            Some((mode, id_)) => id = Some(MapOrScore::Score { mode, id: id_ }),
                            None => return Ok(Err(MAP_PARSE_FAIL.into())),
                        },
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

        Ok(Ok(ScoreArgs { config, mods, id }))
    }
}

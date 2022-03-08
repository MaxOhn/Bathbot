use crate::{
    commands::{osu::option_mods, DoubleResultCow, MyCommand},
    database::OsuData,
    embeds::{EmbedData, LeaderboardEmbed},
    error::Error,
    pagination::{LeaderboardPagination, Pagination},
    util::{
        constants::{
            common_literals::{MAP, MAP_PARSE_FAIL, MODS, MODS_PARSE_FAIL},
            AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE,
        },
        matcher, numbers,
        osu::{map_id_from_history, map_id_from_msg, MapIdType, ModSelection},
        ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use eyre::Report;
use rosu_v2::error::OsuError;
use std::sync::Arc;
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    channel::message::MessageType,
};

use super::option_map;

async fn _leaderboard(
    national: bool,
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: LeaderboardArgs,
) -> BotResult<()> {
    let author_id = data.author()?.id;
    let LeaderboardArgs { map, mods } = args;

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

    let author_name = match ctx.psql().get_user_osu(author_id).await {
        Ok(osu) => osu.map(OsuData::into_username),
        Err(why) => {
            let wrap = format!("failed to get UserConfig of user {author_id}");
            warn!("{:?}", Report::new(why).wrap_err(wrap));

            None
        }
    };

    // Retrieving the beatmap
    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Add map to database if its not in already
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!(
                    "Could not find beatmap with id `{}`. \
                    Did you give me a mapset id instead of a map id?",
                    map_id
                );

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    // Retrieve the map's leaderboard
    let scores_future = ctx.clients.custom.get_leaderboard(
        map_id,
        national,
        match mods {
            Some(ModSelection::Exclude(_)) | None => None,
            Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
        },
        map.mode,
    );

    let scores = match scores_future.await {
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
        .map(|s| format!("{}{}", AVATAR_URL, s.user_id));

    let pages = numbers::div_euclid(10, scores.len());

    let data_fut = LeaderboardEmbed::new(
        author_name.as_deref(),
        &map,
        None,
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().take(10))
        },
        &first_place_icon,
        0,
        &ctx,
        (1, pages),
    );

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sending the embed
    let content =
        format!("I found {amount} scores with the specified mods on the map's leaderboard");

    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

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
        None,
        scores,
        author_name,
        first_place_icon,
        Arc::clone(&ctx),
    );

    let owner = author_id;

    gb.execute(&ctx).await;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display the global leaderboard of a map")]
#[long_desc(
    "Display the global leaderboard of a given map.\n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("lb", "glb", "globalleaderboard")]
async fn leaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match LeaderboardArgs::args(&mut args) {
            Ok(mut leaderboard_args) => {
                let reply = msg
                    .referenced_message
                    .as_ref()
                    .filter(|_| msg.kind == MessageType::Reply);

                if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                    leaderboard_args.map = Some(id);
                }

                let data = CommandData::Message { msg, args, num };

                _leaderboard(false, ctx, data, leaderboard_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_leaderboard(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display the belgian leaderboard of a map")]
#[long_desc(
    "Display the belgian leaderboard of a given map.\n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[aliases("blb")]
async fn belgianleaderboard(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match LeaderboardArgs::args(&mut args) {
            Ok(mut leaderboard_args) => {
                let reply = msg
                    .referenced_message
                    .as_ref()
                    .filter(|_| msg.kind == MessageType::Reply);

                if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                    leaderboard_args.map = Some(id);
                }

                let data = CommandData::Message { msg, args, num };

                _leaderboard(true, ctx, data, leaderboard_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_leaderboard(ctx, *command).await,
    }
}

struct LeaderboardArgs {
    map: Option<MapIdType>,
    mods: Option<ModSelection>,
}

impl LeaderboardArgs {
    fn args(args: &mut Args<'_>) -> Result<Self, String> {
        let mut map = None;
        let mut mods = None;

        for arg in args.take(3) {
            if let Some(id) =
                matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
            {
                map = Some(id);
            } else if let Some(mods_) = matcher::get_mods(arg) {
                mods = Some(mods_);
            } else {
                let content = format!(
                    "Failed to parse `{}`.\n\
                    Must be either a map id, map url, or mods.",
                    arg
                );

                return Err(content);
            }
        }

        Ok(Self { map, mods })
    }

    fn slash(command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let mut map = None;
        let mut mods = None;

        for option in command.yoink_options() {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
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
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let args = Self { map, mods };

        Ok(Ok(args))
    }
}

pub async fn slash_leaderboard(
    ctx: Arc<Context>,
    mut command: ApplicationCommand,
) -> BotResult<()> {
    match LeaderboardArgs::slash(&mut command)? {
        Ok(args) => _leaderboard(false, ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_leaderboard() -> MyCommand {
    let map = option_map();
    let mods = option_mods(true);

    MyCommand::new("leaderboard", "Display the global leaderboard of a map")
        .options(vec![map, mods])
}

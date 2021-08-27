use crate::{
    embeds::{EmbedData, LeaderboardEmbed},
    pagination::{LeaderboardPagination, Pagination},
    util::{
        constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
        matcher,
        osu::{
            cached_message_extract, map_id_from_history, map_id_from_msg, MapIdType, ModSelection,
        },
        ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use rosu_v2::error::OsuError;
use std::{borrow::Cow, sync::Arc};
use twilight_model::{
    application::{
        command::{ChoiceCommandOptionData, Command, CommandOption},
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    channel::message::MessageType,
};

async fn _leaderboard(
    national: bool,
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: LeaderboardArgs,
) -> BotResult<()> {
    let author_id = data.author()?.id;
    let channel_id = data.channel_id();
    let LeaderboardArgs { map, mods } = args;

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

    let author_name = match ctx.user_config(author_id).await {
        Ok(config) => config.name,
        Err(why) => {
            unwind_error!(
                warn,
                why,
                "Failed to get UserConfig of user {}: {}",
                author_id
            );

            None
        }
    };

    // Retrieving the beatmap
    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => map,
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

    // Add map to database if its not in already
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        unwind_error!(warn, why, "Could not add map to DB: {}");
    }

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination =
        LeaderboardPagination::new(response, map, None, scores, author_name, first_place_icon);

    let owner = author_id;

    gb.execute(&ctx).await;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (leaderboard): {}")
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
                    .filter(|msg| msg.kind == MessageType::Reply);

                if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                    leaderboard_args.map = Some(id);
                }

                let data = CommandData::Message { msg, args, num };

                _leaderboard(false, ctx, data, leaderboard_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_leaderboard(ctx, command).await,
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
                    .filter(|msg| msg.kind == MessageType::Reply);

                if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                    leaderboard_args.map = Some(id);
                }

                let data = CommandData::Message { msg, args, num };

                _leaderboard(true, ctx, data, leaderboard_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_leaderboard(ctx, command).await,
    }
}

struct LeaderboardArgs {
    map: Option<MapIdType>,
    mods: Option<ModSelection>,
}

impl LeaderboardArgs {
    const ERR_PARSE_MAP: &'static str = "Failed to parse map url.\n\
        Be sure you specify a valid map id or url to a map.";

    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
            Be sure it's a valid mod abbreviation e.g. `hdhr`.";

    fn args(args: &mut Args) -> Result<Self, String> {
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

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut map = None;
        let mut mods = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "map" => match matcher::get_osu_map_id(&value)
                        .or_else(|| matcher::get_osu_mapset_id(&value))
                    {
                        Some(id) => map = Some(id),
                        None => return Ok(Err(Self::ERR_PARSE_MAP.into())),
                    },
                    "mods" => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => match value.parse() {
                            Ok(mods_) => mods = Some(ModSelection::Exact(mods_)),
                            Err(_) => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                        },
                    },
                    _ => bail_cmd_option!("leaderboard", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("leaderboard", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("leaderboard", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("leaderboard", subcommand, name)
                }
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

pub fn slash_leaderboard_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "leaderboard".to_owned(),
        default_permission: None,
        description: "Display the global leaderboard of a map".to_owned(),
        id: None,
        options: vec![
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify a map url or map id".to_owned(),
                name: "map".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify mods".to_owned(),
                name: "mods".to_owned(),
                required: false,
            }),
        ],
    }
}

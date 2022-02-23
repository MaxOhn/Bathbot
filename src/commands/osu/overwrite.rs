use std::sync::Arc;

use eyre::Report;
use rosu_v2::prelude::{BeatmapsetCompact, GameMode, OsuError};
use tokio::time::{sleep, Duration};
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

use crate::{
    commands::{parse_discord, DoubleResultCow, MyCommand, MyCommandOption},
    database::OsuData,
    embeds::{EmbedData, SimulateEmbed},
    error::Error,
    util::{
        constants::{
            common_literals::{
                ACC, COMBO, DISCORD, MAP_PARSE_FAIL, MISSES, MODS_PARSE_FAIL, NAME, SCORE,
            },
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher,
        osu::{map_id_from_history, MapIdType, ModSelection},
        InteractionExt, MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use super::{
    option_discord, option_map, option_mods, option_name, request_by_map, request_by_score,
    ScoreData, ScoreResult,
};

async fn _override(ctx: Arc<Context>, data: CommandData<'_>, args: OverrideArgs) -> BotResult<()> {
    let name = match args.osu.as_ref().map(OsuData::username) {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let mods = match args.mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) => Some(mods),
    };

    let data_result = match args.id {
        Some(MapOrScore::Score { id, mode }) => {
            request_by_score(&ctx, &data, id, mode, name.as_str()).await
        }
        Some(MapOrScore::Map(MapIdType::Map(id))) => {
            request_by_map(&ctx, &data, id, name.as_str(), mods).await
        }
        Some(MapOrScore::Map(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return data.error(&ctx, content).await;
        }
        None => {
            let msgs = match ctx.retrieve_channel_history(data.channel_id()).await {
                Ok(msgs) => msgs,
                Err(why) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(why);
                }
            };

            match map_id_from_history(&msgs) {
                Some(MapIdType::Map(id)) => {
                    request_by_map(&ctx, &data, id, name.as_str(), mods).await
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

    let ScoreData {
        user,
        map,
        mut scores,
    } = match data_result {
        ScoreResult::Data(data) => data,
        ScoreResult::Done => return Ok(()),
        ScoreResult::Error(err) => return Err(err),
    };

    let map_id = if let Some(id) = args.map {
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

    // Retrieving the beatmap
    let mut map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => map,
            Err(OsuError::NotFound) => {
                let content = format!(
                    "Could not find beatmap with id `{map_id}`. \
                    Did you give me a mapset id instead of a map id?"
                );

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        },
    };

    let mapset: BeatmapsetCompact = map.mapset.take().unwrap().into();

    let maximize = match (args.config.embeds_maximized, data.guild_id()) {
        (Some(embeds_maximized), _) => embeds_maximized,
        (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
        (None, None) => true,
    };

    // Accumulate all necessary data
    let embed_data = match SimulateEmbed::new(None, &map, &mapset, args.into(), &ctx).await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let content = "Simulated score:";

    // Only maximize if config allows it
    if maximize {
        let embed = embed_data.as_builder().build();
        let builder = MessageBuilder::new().content(content).embed(embed);
        let response = data.create_message(&ctx, builder).await?.model().await?;

        ctx.store_msg(response.id);

        // Store map in DB
        if let Err(err) = ctx.psql().insert_beatmap(&map).await {
            warn!("{:?}", Report::new(err));
        }

        // Set map on garbage collection list if unranked
        let gb = ctx.map_garbage_collector(&map);

        // Minimize embed after delay
        tokio::spawn(async move {
            gb.execute(&ctx).await;
            sleep(Duration::from_secs(45)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let embed = embed_data.into_builder().build();
            let builder = MessageBuilder::new().content(content).embed(embed);

            if let Err(why) = response.update_message(&ctx, builder).await {
                let report = Report::new(why).wrap_err("failed to minimize message");
                warn!("{:?}", report);
            }
        });
    } else {
        let embed = embed_data.into_builder().build();
        let builder = MessageBuilder::new().content(content).embed(embed);
        data.create_message(&ctx, builder).await?;

        // Store map in DB, combo was inserted earlier
        if let Err(err) = ctx.psql().insert_beatmap(&map).await {
            warn!("{:?}", Report::new(err));
        }

        // Set map on garbage collection list if unranked
        ctx.map_garbage_collector(&map).execute(&ctx).await;
    }

    Ok(())
}

enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

pub struct OverrideArgs {
    osu: Option<OsuData>,
    id: Option<MapOrScore>,
    pub mods: Option<ModSelection>,
    pub n300: Option<usize>,
    pub n100: Option<usize>,
    pub n50: Option<usize>,
    pub misses: Option<usize>,
    pub acc: Option<f32>,
    pub combo: Option<usize>,
    pub score: Option<u32>,
}

impl OverrideArgs {
    async fn slash(ctx: &Context, command: &ApplicationCommand) -> DoubleResultCow<Self> {
        let mut osu = ctx.psql().get_user_osu(command.user_id()?).await?;
        let mut id = None;
        let mut mods = None;
        let mut n300 = None;
        let mut n100 = None;
        let mut n50 = None;
        let mut misses = None;
        let mut acc = None;
        let mut combo = None;
        let mut score = None;

        for option in &command.data.options {
            match &option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => osu = Some(OsuData::Name(value.as_str().into())),
                    MAP => match matcher::get_osu_map_id(value)
                        .or_else(|| matcher::get_osu_mapset_id(value))
                    {
                        Some(id_) => id = Some(MapOrScore::Map(id_)),
                        None => match matcher::get_osu_score_id(&value) {
                            Some((mode, id_)) => id = Some(MapOrScore::Score { mode, id: id_ }),
                            None => return Ok(Err(MAP_PARSE_FAIL.into())),
                        },
                    },
                    MODS => match matcher::get_mods(value) {
                        Some(mods_) => mods = Some(mods_),
                        None => match value.parse() {
                            Ok(mods_) => mods = Some(ModSelection::Exact(mods_)),
                            Err(_) => return Ok(Err(MODS_PARSE_FAIL.into())),
                        },
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => match option.name.as_str() {
                    "n300" => n300 = Some(*value.max(&0) as usize),
                    "n100" => n100 = Some(*value.max(&0) as usize),
                    "n50" => n50 = Some(*value.max(&0) as usize),
                    MISSES => misses = Some(*value.max(&0) as usize),
                    COMBO => combo = Some(*value.max(&0) as usize),
                    SCORE => score = Some(*value.max(&0) as u32),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Number(value) => match option.name.as_str() {
                    ACC => acc = Some(value.0.clamp(0.0, 100.0) as f32),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, *value).await? {
                        Ok(osu_) => osu = Some(osu_),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let args = Self {
            osu,
            id,
            mods,
            n300,
            n100,
            n50,
            misses,
            acc,
            combo,
            score,
        };

        Ok(Ok(args))
    }
}

pub async fn slash_override(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    match OverrideArgs::slash(&ctx, &command).await? {
        Ok(args) => _override(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_override() -> MyCommand {
    let name = option_name();
    let map = option_map();
    let mods = option_mods(false);

    let n300 = MyCommandOption::builder("n300", "Specify the amount of 300s")
        .min_int(0)
        .integer(Vec::new(), false);

    let n100 = MyCommandOption::builder("n100", "Specify the amount of 100s")
        .min_int(0)
        .integer(Vec::new(), false);

    let n50 = MyCommandOption::builder("n50", "Specify the amount of 50s")
        .min_int(0)
        .integer(Vec::new(), false);

    let misses = MyCommandOption::builder(MISSES, "Specify the amount of misses")
        .min_int(0)
        .integer(Vec::new(), false);

    let acc = MyCommandOption::builder(ACC, "Specify the accuracy")
        .help("Specify the accuracy. Should be between 0.0 and 100.0")
        .min_num(0.0)
        .max_num(100.0)
        .number(Vec::new(), false);

    let combo = MyCommandOption::builder(COMBO, "Specify the combo")
        .min_int(0)
        .integer(Vec::new(), false);

    let score_help = "Specifying the score is only necessary for mania.\n\
        The value should be between 0 and 1,000,000 and already adjusted to mods \
        e.g. only up to 500,000 for `EZ` or up to 250,000 for `EZNF`.";

    let score = MyCommandOption::builder(SCORE, "Specify the score")
        .help(score_help)
        .min_int(0)
        .integer(Vec::new(), false);

    let discord = option_discord();

    let help = "Simulate a score and check if it changes the user's pp.\n\
        Note that hitresults, combo, and accuracy are ignored in mania; only score is important.";

    let description = "Simulate a score and check if it changes the user's pp";
    let options = vec![
        name, map, mods, n300, n100, n50, misses, combo, acc, score, discord,
    ];

    MyCommand::new("override", description)
        .help(help)
        .options(options)
}

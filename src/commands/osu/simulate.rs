use crate::{
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher,
        osu::{map_id_from_history, map_id_from_msg, MapIdType, ModSelection},
        ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use rosu_v2::prelude::{BeatmapsetCompact, OsuError};
use std::{borrow::Cow, sync::Arc};
use tokio::time::{self, Duration};
use twilight_model::{
    application::{
        command::{ChoiceCommandOptionData, Command, CommandOption},
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    channel::message::MessageType,
};

#[command]
#[short_desc("Simulate a score on a map")]
#[long_desc(
    "Simulate a (perfect) score on the given map. \
     Mods can be specified with `+mods` e.g. `+hdhr`.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    For the keys `n300`, `n100`, `n50`, `misses`, `combo`, and `score` you must \
    specify an interger value.\n\
    For the `acc` key you must specify a number between 0.0 and 100.0.\n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     The `score` option is only relevant for mania."
)]
#[usage(
    "[map url / map id] [+mods] [acc=number] [combo=integer] [n300=integer] \
    [n100=integer] [n50=integer] [misses=integer] [score=integer]"
)]
#[example(
    "1980365 +hddt acc=99.3 combo=1234 n300=1422 n50=2 misses=1",
    "https://osu.ppy.sh/beatmapsets/948199#osu/1980365 acc=97.56"
)]
#[aliases("s")]
async fn simulate(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match SimulateArgs::args(&mut args) {
            Ok(mut simulate_args) => {
                let reply = msg
                    .referenced_message
                    .as_ref()
                    .filter(|_| msg.kind == MessageType::Reply);

                if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                    simulate_args.map = Some(id);
                }

                _simulate(ctx, CommandData::Message { msg, args, num }, simulate_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_simulate(ctx, *command).await,
    }
}

async fn _simulate(ctx: Arc<Context>, data: CommandData<'_>, args: SimulateArgs) -> BotResult<()> {
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

    let mapset: BeatmapsetCompact = map.mapset.take().unwrap().into();

    // Accumulate all necessary data
    let embed_data = match SimulateEmbed::new(None, &map, &mapset, args.into()).await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let embed = embed_data.as_builder().build();
    let content = "Simulated score:";
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response = data.create_message(&ctx, builder).await?.model().await?;

    ctx.store_msg(response.id);

    // Add map to database if its not in already
    if let Err(why) = ctx.psql().insert_beatmap(&map).await {
        unwind_error!(warn, why, "Could not add map to DB: {}");
    }

    // Set map on garbage collection list if unranked
    let gb = ctx.map_garbage_collector(&map);

    // Minimize embed after delay
    tokio::spawn(async move {
        gb.execute(&ctx).await;
        time::sleep(Duration::from_secs(45)).await;

        if !ctx.remove_msg(response.id) {
            return;
        }

        let embed = embed_data.into_builder().build();
        let builder = MessageBuilder::new().content(content).embed(embed);

        if let Err(why) = response.update_message(&ctx, builder).await {
            unwind_error!(warn, why, "Error minimizing simulate msg: {}");
        }
    });

    Ok(())
}

pub struct SimulateArgs {
    map: Option<MapIdType>,
    pub mods: Option<ModSelection>,
    pub n300: Option<usize>,
    pub n100: Option<usize>,
    pub n50: Option<usize>,
    pub misses: Option<usize>,
    pub acc: Option<f32>,
    pub combo: Option<usize>,
    pub score: Option<u32>,
}

macro_rules! parse_fail {
    ($key:ident, $ty:literal) => {
        return Err(format!(
            concat!("Failed to parse `{}`. Must be ", $ty, "."),
            $key
        ));
    };
}

impl SimulateArgs {
    const ERR_PARSE_MAP: &'static str = "Failed to parse map url.\n\
        Be sure you specify a valid map id or url to a map.";

    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        Be sure it's a valid mod abbreviation e.g. `hdhr`.";

    fn args(args: &mut Args) -> Result<Self, String> {
        let mut map = None;
        let mut mods = None;
        let mut n300 = None;
        let mut n100 = None;
        let mut n50 = None;
        let mut misses = None;
        let mut acc = None;
        let mut combo = None;
        let mut score = None;

        for arg in args {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = &arg[idx + 1..];

                match key {
                    "n300" => match value.parse() {
                        Ok(value) => n300 = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "n100" => match value.parse() {
                        Ok(value) => n100 = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "n50" => match value.parse() {
                        Ok(value) => n50 = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "misses" | "miss" | "m" => match value.parse() {
                        Ok(value) => misses = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "acc" | "a" | "accuracy" => match value.parse() {
                        Ok(value) => acc = Some(value),
                        Err(_) => parse_fail!(key, "a number"),
                    },
                    "combo" | "c" => match value.parse() {
                        Ok(value) => combo = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "score" | "s" => match value.parse() {
                        Ok(value) => score = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    "mods" => match value.parse() {
                        Ok(m) => mods = Some(ModSelection::Exact(m)),
                        Err(_) => parse_fail!(key, "a valid mod abbreviation"),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `n300`, `n100`, `n50`, \
                            `misses`, `acc`, `combo`, and `score`.",
                            key
                        );

                        return Err(content);
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(arg) {
                mods.replace(mods_);
            } else if let Some(id) =
                matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
            {
                map = Some(id);
            } else {
                let content = format!(
                    "Failed to parse `{}`.\n\
                    Be sure to specify either of the following: map id, map url, mods, or \
                    options in the form `key=value`.\nCheck the command's help for more info.",
                    arg
                );

                return Err(content);
            }
        }

        let args = Self {
            map,
            mods,
            n300,
            n100,
            n50,
            misses,
            acc,
            combo,
            score,
        };

        Ok(args)
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut map = None;
        let mut mods = None;
        let mut n300 = None;
        let mut n100 = None;
        let mut n50 = None;
        let mut misses = None;
        let mut acc = None;
        let mut combo = None;
        let mut score = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
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
                    "acc" => match value.parse::<f32>() {
                        Ok(num) => acc = Some(num.max(0.0).min(100.0)),
                        Err(_) => {
                            let content = "Failed to parse `acc`. Must be a number.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => bail_cmd_option!("simulate", string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    "n300" => n300 = Some(value.max(0) as usize),
                    "n100" => n100 = Some(value.max(0) as usize),
                    "n50" => n50 = Some(value.max(0) as usize),
                    "misses" => misses = Some(value.max(0) as usize),
                    "combo" => combo = Some(value.max(0) as usize),
                    "score" => score = Some(value.max(0) as u32),
                    _ => bail_cmd_option!("simulate", integer, name),
                },
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("simulate", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("simulate", subcommand, name)
                }
            }
        }

        let args = Self {
            map,
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

pub async fn slash_simulate(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match SimulateArgs::slash(&mut command)? {
        Ok(args) => _simulate(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_simulate_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "simulate".to_owned(),
        default_permission: None,
        description: "Simulate a score on a map".to_owned(),
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
                description: "Specify mods e.g. hdhr or nm".to_owned(),
                name: "mods".to_owned(),
                required: false,
            }),
            CommandOption::Integer(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the amount of 300s".to_owned(),
                name: "n300".to_owned(),
                required: false,
            }),
            CommandOption::Integer(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the amount of 100s".to_owned(),
                name: "n100".to_owned(),
                required: false,
            }),
            CommandOption::Integer(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the amount of 50s".to_owned(),
                name: "n50".to_owned(),
                required: false,
            }),
            CommandOption::Integer(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the amount of misses".to_owned(),
                name: "misses".to_owned(),
                required: false,
            }),
            // TODO: Number
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the accuracy".to_owned(),
                name: "acc".to_owned(),
                required: false,
            }),
            CommandOption::Integer(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the combo".to_owned(),
                name: "combo".to_owned(),
                required: false,
            }),
            CommandOption::Integer(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the score (only relevant for mania)".to_owned(),
                name: "score".to_owned(),
                required: false,
            }),
        ],
    }
}

use crate::{
    commands::{MyCommand, MyCommandOption},
    embeds::{EmbedData, SimulateEmbed},
    util::{
        constants::{
            common_literals::{
                ACC, ACCURACY, COMBO, MAP, MAP_PARSE_FAIL, MISSES, MODS, MODS_PARSE_FAIL, SCORE,
            },
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher,
        osu::{map_id_from_history, map_id_from_msg, MapIdType, ModSelection},
        ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

use eyre::Report;
use rosu_v2::prelude::{BeatmapsetCompact, OsuError};
use std::{borrow::Cow, sync::Arc};
use tokio::time::{self, Duration};
use twilight_model::{
    application::interaction::{application_command::CommandDataOption, ApplicationCommand},
    channel::message::MessageType,
};

use super::{option_map, option_mods};

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
    if let Err(err) = ctx.psql().insert_beatmap(&map).await {
        warn!("{:?}", Report::new(err));
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
            let report = Report::new(why).wrap_err("failed to minimize simulate msg");
            warn!("{:?}", report);
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
        ))
    };
}

const SIMULATE: &str = "simulate";

impl SimulateArgs {
    fn args(args: &mut Args<'_>) -> Result<Self, String> {
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
                    MISSES | "miss" | "m" => match value.parse() {
                        Ok(value) => misses = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    ACC | "a" | ACCURACY => match value.parse() {
                        Ok(value) => acc = Some(value),
                        Err(_) => parse_fail!(key, "a number"),
                    },
                    COMBO | "c" => match value.parse() {
                        Ok(value) => combo = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    SCORE | "s" => match value.parse() {
                        Ok(value) => score = Some(value),
                        Err(_) => parse_fail!(key, "a positive integer"),
                    },
                    MODS => match value.parse() {
                        Ok(m) => mods = Some(ModSelection::Exact(m)),
                        Err(_) => return Err(MODS_PARSE_FAIL.to_owned()),
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
                    ACC => match value.parse::<f32>() {
                        Ok(num) => acc = Some(num.max(0.0).min(100.0)),
                        Err(_) => {
                            let content = "Failed to parse `acc`. Must be a number.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => bail_cmd_option!(SIMULATE, string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    "n300" => n300 = Some(value.max(0) as usize),
                    "n100" => n100 = Some(value.max(0) as usize),
                    "n50" => n50 = Some(value.max(0) as usize),
                    MISSES => misses = Some(value.max(0) as usize),
                    COMBO => combo = Some(value.max(0) as usize),
                    SCORE => score = Some(value.max(0) as u32),
                    _ => bail_cmd_option!(SIMULATE, integer, name),
                },
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(SIMULATE, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(SIMULATE, subcommand, name)
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

pub fn define_simulate() -> MyCommand {
    let map = option_map();
    let mods = option_mods(false);

    let n300 =
        MyCommandOption::builder("n300", "Specify the amount of 300s").integer(Vec::new(), false);

    let n100 =
        MyCommandOption::builder("n100", "Specify the amount of 100s").integer(Vec::new(), false);

    let n50 =
        MyCommandOption::builder("n50", "Specify the amount of 50s").integer(Vec::new(), false);

    let misses =
        MyCommandOption::builder(MISSES, "Specify the amount of misses").integer(Vec::new(), false);

    // TODO: Number variant
    let acc = MyCommandOption::builder(ACC, "Specify the accuracy")
        .help("Specify the accuracy. Should be between 0.0 and 100.0")
        .string(Vec::new(), false);

    let combo = MyCommandOption::builder(COMBO, "Specify the combo").integer(Vec::new(), false);

    let score_help = "Specifying the score is only necessary for mania.\n\
        The value should be between 0 and 1,000,000 and already adjusted to mods \
        e.g. only up to 500,000 for `EZ` or up to 250,000 for `EZNF`.";

    let score = MyCommandOption::builder(SCORE, "Specify the score")
        .help(score_help)
        .integer(Vec::new(), false);

    let help = "Simulate a score on a map.\n\
        Note that hitresults, combo, and accuracy are ignored in mania; only score is important.";

    MyCommand::new(SIMULATE, "Simulate a score on a map")
        .help(help)
        .options(vec![map, mods, n300, n100, n50, misses, combo, acc, score])
}

use std::sync::Arc;

use command_macros::{command, HasMods, SlashCommand};
use eyre::Report;
use rosu_v2::prelude::{BeatmapsetCompact, OsuError};
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::{message::MessageType, Message},
};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    database::EmbedsSize,
    embeds::{EmbedData, SimulateEmbed},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher,
        osu::{map_id_from_history, map_id_from_msg, MapIdType, ModSelection},
        ApplicationCommandExt, ChannelExt, CowUtils, MessageExt,
    },
    BotResult, Context,
};

use super::{HasMods, ModsResult};

#[derive(CommandModel, CreateCommand, HasMods, SlashCommand)]
#[command(
    name = "simulate",
    help = "Simulate a score on a map.\n\
    Note that hitresults, combo, and accuracy are ignored in mania; only score is important."
)]
/// Simulate a score on a map
pub struct Simulate {
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<String>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: Option<String>,
    #[command(min_value = 0)]
    /// Specify the amount of 300s
    n300: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 100s
    n100: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 50s
    n50: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of misses
    misses: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the combo
    combo: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify the accuracy
    acc: Option<f32>,
    #[command(
        min_value = 0,
        max_value = 1_000_000,
        help = "Specifying the score is only necessary for mania.\n\
        The value should be between 0 and 1,000,000 and already adjusted to mods \
        e.g. only up to 500,000 for `EZ` or up to 250,000 for `EZNF`."
    )]
    /// Specify the score
    score: Option<u32>,
}

impl TryFrom<Simulate> for SimulateArgs {
    type Error = &'static str;

    fn try_from(args: Simulate) -> Result<Self, Self::Error> {
        let mods = match args.mods() {
            ModsResult::Mods(mods) => Some(mods),
            ModsResult::None => None,
            ModsResult::Invalid => {
                return Err(
                    "Failed to parse mods. Be sure to either specify them directly \
                    or through the `+mods` / `+mods!` syntax e.g. `hdhr` or `+hdhr!`",
                )
            }
        };

        let map = match args.map {
            Some(map) => {
                if let Some(id) =
                    matcher::get_osu_map_id(&map).or_else(|| matcher::get_osu_mapset_id(&map))
                {
                    Some(id)
                } else {
                    return Err(
                        "Failed to parse map url. Be sure you specify a valid map id or url to a map.",
                    );
                }
            }
            None => None,
        };

        Ok(Self {
            map,
            mods,
            n300: args.n300.map(|n| n as usize),
            n100: args.n100.map(|n| n as usize),
            n50: args.n50.map(|n| n as usize),
            misses: args.misses.map(|n| n as usize),
            acc: args.acc,
            combo: args.combo.map(|n| n as usize),
            score: args.score,
        })
    }
}

#[command]
#[desc("Simulate a score on a map")]
#[help(
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
#[alias("s")]
#[group(AllModes)]
async fn prefix_simulate(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match SimulateArgs::args(msg, args) {
        Ok(args) => simulate(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_simulate(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Simulate::from_interaction(command.input_data())?;

    match SimulateArgs::try_from(args) {
        Ok(args) => simulate(ctx, command.into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn simulate(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: SimulateArgs) -> BotResult<()> {
    let map_id = if let Some(id) = args.map {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
            Ok(msgs) => msgs,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        };

        match map_id_from_history(&msgs) {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                return orig.error(&ctx, content).await;
            }
        }
    };

    let map_id = match map_id {
        MapIdType::Map(id) => id,
        MapIdType::Set(_) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(&ctx, content).await;
        }
    };

    let map_fut = ctx.psql().get_beatmap(map_id, true);
    let config_fut = ctx.user_config(orig.user_id()?);

    let (map_result, config_result) = tokio::join!(map_fut, config_fut);

    // Retrieving the beatmap
    let mut map = match map_result {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Store map in DB
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!(
                    "Could not find beatmap with id `{map_id}`. \
                    Did you give me a mapset id instead of a map id?"
                );

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
        },
    };

    let mapset: BeatmapsetCompact = map.mapset.take().unwrap().into();

    let embeds_size = match config_result {
        Ok(config) => config.embeds_size,
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to get user config");
            warn!("{report:?}");

            None
        }
    };

    let embeds_size = match (embeds_size, orig.guild_id()) {
        (Some(size), _) => size,
        (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
        (None, None) => EmbedsSize::default(),
    };

    // Accumulate all necessary data
    let embed_data = match SimulateEmbed::new(None, &map, &mapset, args.into(), &ctx).await {
        Ok(data) => data,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let content = "Simulated score:";

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let embed = embed_data.into_builder().build();
            let builder = MessageBuilder::new().content(content).embed(embed);
            orig.create_message(&ctx, &builder).await?;
        }
        EmbedsSize::InitialMaximized => {
            let embed = embed_data.as_builder().build();
            let builder = MessageBuilder::new().content(content).embed(embed);
            let response = orig.create_message(&ctx, &builder).await?.model().await?;

            ctx.store_msg(response.id);
            let ctx = Arc::clone(&ctx);

            // Minimize embed after delay
            tokio::spawn(async move {
                sleep(Duration::from_secs(45)).await;

                if !ctx.remove_msg(response.id) {
                    return;
                }

                let embed = embed_data.into_builder().build();
                let builder = MessageBuilder::new().content(content).embed(embed);

                if let Err(err) = response.update(&ctx, &builder).await {
                    let report = Report::new(err).wrap_err("failed to minimize message");
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let embed = embed_data.as_builder().build();
            let builder = MessageBuilder::new().content(content).embed(embed);
            orig.create_message(&ctx, &builder).await?;
        }
    }

    // Set map on garbage collection list if unranked
    ctx.map_garbage_collector(&map).execute(&ctx);

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

impl SimulateArgs {
    fn args(msg: &Message, args: Args<'_>) -> Result<Self, String> {
        let mut map = None;
        let mut mods = None;
        let mut n300 = None;
        let mut n100 = None;
        let mut n50 = None;
        let mut misses = None;
        let mut acc = None;
        let mut combo = None;
        let mut score = None;

        for arg in args.map(|arg| arg.cow_to_ascii_lowercase()) {
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
                        Err(_) => return Err("Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.".to_owned()),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `n300`, `n100`, `n50`, \
                            `misses`, `acc`, `combo`, and `score`."
                        );

                        return Err(content);
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(&arg) {
                mods = Some(mods_);
            } else if let Some(id) =
                matcher::get_osu_map_id(&arg).or_else(|| matcher::get_osu_mapset_id(&arg))
            {
                map = Some(id);
            } else {
                let content = format!(
                    "Failed to parse `{arg}`.\n\
                    Be sure to specify either of the following: map id, map url, mods, or \
                    options in the form `key=value`.\nCheck the command's help for more info."
                );

                return Err(content);
            }
        }

        let reply = msg
            .referenced_message
            .as_ref()
            .filter(|_| msg.kind == MessageType::Reply);

        if let Some(reply) = reply {
            if let Some(map_) = map_id_from_msg(&reply) {
                map = Some(map_);
            }
        }

        Ok(Self {
            map,
            mods,
            n300,
            n100,
            n50,
            misses,
            acc,
            combo,
            score,
        })
    }
}

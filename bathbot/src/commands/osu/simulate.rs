use std::sync::Arc;

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::ScoreSize;
use bathbot_util::{
    constants::GENERAL_ISSUE,
    matcher,
    osu::{MapIdType, ModSelection},
    CowUtils, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::GameMods;
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::channel::{message::MessageType, Message};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::SimulateEmbed,
    manager::{MapError, OsuMap},
    util::{
        interaction::InteractionCommand, osu::IfFc, ChannelExt, InteractionCommandExt, MessageExt,
    },
    Context,
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
                if let Some(id) = matcher::get_osu_map_id(&map)
                    .map(MapIdType::Map)
                    .or_else(|| matcher::get_osu_mapset_id(&map).map(MapIdType::Set))
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
async fn prefix_simulate(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match SimulateArgs::args(msg, args) {
        Ok(args) => simulate(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_simulate(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Simulate::from_interaction(command.input_data())?;

    match SimulateArgs::try_from(args) {
        Ok(args) => simulate(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn simulate(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: SimulateArgs) -> Result<()> {
    let map_id = match args.map {
        Some(MapIdType::Map(id)) => id,
        Some(MapIdType::Set(_)) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(&ctx, content).await;
        }
        None => {
            let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("failed to retrieve channel history"));
                }
            };

            match MapIdType::map_from_msgs(&msgs, 0) {
                Some(id) => id,
                None => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return orig.error(&ctx, content).await;
                }
            }
        }
    };

    let map_fut = ctx.osu_map().map(map_id, None);
    let score_size_fut = ctx.user_config().score_size(orig.user_id()?);

    let (map_res, score_size_res) = tokio::join!(map_fut, score_size_fut);

    let map = match map_res {
        Ok(map) => map,
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?"
            );

            return orig.error(&ctx, content).await;
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mods = match args.mods {
        Some(ModSelection::Exact(mods) | ModSelection::Include(mods)) => mods,
        _ => GameMods::NoMod,
    };

    let attrs = ctx.pp(&map).mods(mods).mode(map.mode()).performance().await;

    let entry = SimulateEntry {
        original_score: None,
        if_fc: None,
        map,
        stars: attrs.stars() as f32,
        max_pp: attrs.pp() as f32,
    };

    let score_size = match score_size_res {
        Ok(Some(score_size)) => score_size,
        Ok(None) => match orig.guild_id() {
            Some(guild_id) => ctx
                .guild_config()
                .peek(guild_id, |config| config.score_size)
                .await
                .unwrap_or_default(),
            None => ScoreSize::default(),
        },
        Err(err) => {
            warn!("{:?}", err.wrap_err("failed to get user score size"));

            ScoreSize::default()
        }
    };

    // Accumulate all necessary data
    let embed_data = SimulateEmbed::new(&entry, args.into(), &ctx).await;
    let content = "Simulated score:";

    // Only maximize if config allows it
    match score_size {
        ScoreSize::AlwaysMinimized => {
            let embed = embed_data.into_minimized();
            let builder = MessageBuilder::new().content(content).embed(embed);
            orig.create_message(&ctx, &builder).await?;
        }
        ScoreSize::InitialMaximized => {
            let embed = embed_data.as_maximized();
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

                let embed = embed_data.into_minimized();
                let builder = MessageBuilder::new().content(content).embed(embed);

                if let Err(err) = response.update(&ctx, &builder).await {
                    let err = Report::new(err).wrap_err("failed to minimize message");
                    warn!("{err:?}");
                }
            });
        }
        ScoreSize::AlwaysMaximized => {
            let embed = embed_data.as_maximized();
            let builder = MessageBuilder::new().content(content).embed(embed);
            orig.create_message(&ctx, &builder).await?;
        }
    }

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
                    "acc" | "a" | "accuracy" => match value.parse::<f32>() {
                        Ok(value) => acc = Some(value.clamp(0.0, 100.0)),
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
            } else if let Some(id) = matcher::get_osu_map_id(&arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(&arg).map(MapIdType::Set))
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
            .as_deref()
            .filter(|_| msg.kind == MessageType::Reply);

        if let Some(map_) = reply.and_then(MapIdType::from_msg) {
            map = Some(map_);
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

pub struct SimulateEntry {
    pub original_score: Option<ScoreSlim>,
    pub if_fc: Option<IfFc>,
    pub map: OsuMap,
    pub stars: f32,
    pub max_pp: f32,
}

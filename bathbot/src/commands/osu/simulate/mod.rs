pub mod args;

use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_util::{constants::GENERAL_ISSUE, matcher, osu::MapIdType};
use eyre::Result;
use rosu_pp::GameMode as Mode;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    commands::GameModeOption,
    core::{
        commands::{prefix::Args, CommandOrigin},
        Context,
    },
    embeds::{SimulateData, TopOldVersion},
    manager::MapError,
    pagination::SimulatePagination,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

use self::args::{ParseError, SimulateArg};

use super::{
    HasMods, ModsResult, TopOldCatchVersion, TopOldManiaVersion, TopOldOsuVersion,
    TopOldTaikoVersion,
};

#[derive(CreateCommand, CommandModel, Default, HasMods, SlashCommand)]
#[command(name = "simulate")]
/// Simulate a score on a map
pub struct Simulate<'m> {
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<Cow<'m, str>>,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify mods
    mods: Option<Cow<'m, str>>,
    /// Specify a combo
    combo: Option<u32>,
    /// Specify an accuracy
    acc: Option<f32>,
    /// Specify a custom clock rate that overwrites mods
    clock_rate: Option<f32>,
    /// Specify the amount of 300s
    n300: Option<u32>,
    /// Specify the amount of 100s
    n100: Option<u32>,
    /// Specify the amount of 50s
    n50: Option<u32>,
    /// Specify misses
    misses: Option<u32>,
    /// Specify gekis i.e. n320 in mania
    geki: Option<u32>,
    /// Specify katus i.e. tiny droplet misses in catch and n200 in mania
    katu: Option<u32>,
    /// Overwrite the map's approach rate
    ar: Option<f32>,
    /// Overwrite the map's circle size
    cs: Option<f32>,
    /// Overwrite the map's drain rate
    hp: Option<f32>,
    /// Overwrite the map's overall difficulty
    od: Option<f32>,
}

impl<'m> Simulate<'m> {
    fn from_args(mode: Option<GameMode>, args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut simulate = Self {
            mode: mode.map(GameModeOption::from),
            ..Default::default()
        };

        for arg in args {
            if matcher::get_osu_map_id(arg).is_some() {
                simulate.map = Some(Cow::Borrowed(arg));

                continue;
            }

            match SimulateArg::parse(arg).map_err(ParseError::to_str)? {
                SimulateArg::Acc(val) => simulate.acc = Some(val),
                SimulateArg::Combo(val) => simulate.combo = Some(val),
                SimulateArg::ClockRate(val) => simulate.clock_rate = Some(val),
                SimulateArg::N300(val) => simulate.n300 = Some(val),
                SimulateArg::N100(val) => simulate.n100 = Some(val),
                SimulateArg::N50(val) => simulate.n50 = Some(val),
                SimulateArg::Geki(val) => simulate.geki = Some(val),
                SimulateArg::Katu(val) => simulate.katu = Some(val),
                SimulateArg::Miss(val) => simulate.misses = Some(val),
                SimulateArg::Mods(val) => simulate.mods = Some(Cow::Borrowed(val)),
                SimulateArg::Ar(val) => simulate.ar = Some(val),
                SimulateArg::Cs(val) => simulate.cs = Some(val),
                SimulateArg::Hp(val) => simulate.hp = Some(val),
                SimulateArg::Od(val) => simulate.od = Some(val),
            }
        }

        Ok(simulate)
    }
}

pub async fn slash_simulate(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Simulate::from_interaction(command.input_data())?;
    let orig = CommandOrigin::from(&mut command);

    simulate(ctx, orig, args).await
}

async fn simulate(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Simulate<'_>) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods.mods()),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods. Be sure to either specify them directly \
            or through the `+mods` / `+mods!` syntax e.g. `hdhr` or `+hdhr!`";

            return orig.error(&ctx, content).await;
        }
    };

    let mode = args.mode.map(GameMode::from);

    let map = match args.map {
        Some(map) => {
            if let Some(id) = matcher::get_osu_map_id(&map)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(&map).map(MapIdType::Set))
            {
                Some(id)
            } else {
                let content =
                    "Failed to parse map url. Be sure you specify a valid map id or url to a map.";

                return orig.error(&ctx, content).await;
            }
        }
        None => None,
    };

    let map_id = match map {
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

    let mut map = match ctx.osu_map().map(map_id, None).await {
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

    let mode = mode.unwrap_or_else(|| map.mode());

    let (version, converted_mode) = match mode {
        GameMode::Osu => (
            TopOldVersion::Osu(TopOldOsuVersion::September22Now),
            Mode::Osu,
        ),
        GameMode::Taiko => (
            TopOldVersion::Taiko(TopOldTaikoVersion::September22Now),
            Mode::Taiko,
        ),
        GameMode::Catch => (
            TopOldVersion::Catch(TopOldCatchVersion::May20Now),
            Mode::Catch,
        ),
        GameMode::Mania => (
            TopOldVersion::Mania(TopOldManiaVersion::October22Now),
            Mode::Mania,
        ),
    };

    match map.pp_map.convert_mode(converted_mode) {
        Cow::Borrowed(_) => {
            // converted ctb maps don't have the internal mode adjusted so
            // let's do that manually in case it's important to have later on
            if mode == GameMode::Osu && converted_mode == Mode::Catch {
                map.pp_map.mode = Mode::Catch;
            }
        }
        Cow::Owned(converted) => map.pp_map = converted,
    }

    let simulate_data = SimulateData {
        mods,
        acc: args.acc,
        n_geki: args.geki,
        n_katu: args.katu,
        n300: args.n300,
        n100: args.n100,
        n50: args.n50,
        n_miss: args.misses,
        combo: args.combo,
        clock_rate: args.clock_rate,
        ar: args.ar,
        cs: args.cs,
        hp: args.hp,
        od: args.od,
        score: None,
        version,
    };

    SimulatePagination::builder(map, simulate_data)
        .content("Simulated score:")
        .simulate_components(version)
        .start_by_update()
        .start(ctx, orig)
        .await
}

#[command]
#[desc("Simulate a score on a map")]
#[help(
    "Simulate a score on the given map.\n\
    If no map is specified by either url or id, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Various arguments can be specified in multiple ways:\n\
    - Accuracy: `acc=[number]` or `[number]%`\n\
    - Combo: `combo=[integer]` or `[integer]x`\n\
    - Clock rate: `clockrate=[number]` or `[number]*`\n\
    - n300: `n300=[integer]` or `[integer]x300`\n\
    - n100: `n100=[interger]` or `[integer]x100`\n\
    - n50: `n50=[integer]` or `[integer]x50`\n\
    - misses: `miss=[integer]` or `[integer]m`\n\
    - gekis (n320): `gekis=[integer]` or `[integer]xgeki`\n\
    - katus (n200 / tiny droplet misses): `katus=[integer]` or `[integer]xkatu`\n\
    - mods: `mods=[mod acronym]` or `+[mod acronym]`\n\
    - ar: `ar=[number]` or `ar[number]`\n\
    - cs: `cs=[number]` or `cs[number]`\n\
    - hp: `hp=[number]` or `hp[number]`\n\
    - od: `od=[number]` or `od[number]`"
)]
#[usage(
    "[map url / map id] [+mods] [acc%] [combox] [clockrate*] \
    [n300x300] [n100x100] [n50x50] [missesm] [gekisxgeki] [katusxkatus]"
)]
#[example("1980365 +hdhr 4000x 1m 2499x300 99.1% 1.05*")]
#[alias("s", "sim")]
#[group(Osu)]
async fn prefix_simulate(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let orig = CommandOrigin::from(msg);

    match Simulate::from_args(None, args) {
        Ok(args) => simulate(ctx, orig, args).await,
        Err(content) => orig.error(&ctx, content).await,
    }
}

#[command]
#[desc("Simulate a taiko score on a map")]
#[help(
    "Simulate a taiko score on the given map.\n\
    If no map is specified by either url or id, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Various arguments can be specified in multiple ways:\n\
    - Accuracy: `acc=[number]` or `[number]%`\n\
    - Combo: `combo=[integer]` or `[integer]x`\n\
    - Clock rate: `clockrate=[number]` or `[number]*`\n\
    - n300: `n300=[integer]` or `[integer]x300`\n\
    - n100: `n100=[interger]` or `[integer]x100`\n\
    - misses: `miss=[integer]` or `[integer]m`\n\
    - mods: `mods=[mod acronym]` or `+[mod acronym]`\n\
    - ar: `ar=[number]` or `ar[number]`\n\
    - cs: `cs=[number]` or `cs[number]`\n\
    - hp: `hp=[number]` or `hp[number]`\n\
    - od: `od=[number]` or `od[number]`"
)]
#[usage(
    "[map url / map id] [+mods] [acc%] [combox] [clockrate*] \
    [n300x300] [n100x100] [missesm]"
)]
#[example("1980365 +hdhr 4000x 1m 2499x300 99.1% 1.05*")]
#[alias("st", "simt", "simtaiko")]
#[group(Taiko)]
async fn prefix_simulatetaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let orig = CommandOrigin::from(msg);

    match Simulate::from_args(Some(GameMode::Taiko), args) {
        Ok(args) => simulate(ctx, orig, args).await,
        Err(content) => orig.error(&ctx, content).await,
    }
}

#[command]
#[desc("Simulate a ctb score on a map")]
#[help(
    "Simulate a ctb score on the given map.\n\
    If no map is specified by either url or id, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Various arguments can be specified in multiple ways:\n\
    - Accuracy: `acc=[number]` or `[number]%`\n\
    - Combo: `combo=[integer]` or `[integer]x`\n\
    - Clock rate: `clockrate=[number]` or `[number]*`\n\
    - fruits: `n300=[integer]` or `[integer]x300`\n\
    - droplets: `n100=[interger]` or `[integer]x100`\n\
    - tiny droplets: `n50=[integer]` or `[integer]x50`\n\
    - misses: `miss=[integer]` or `[integer]m`\n\
    - tiny droplet misses: `katus=[integer]` or `[integer]xkatu`\n\
    - mods: `mods=[mod acronym]` or `+[mod acronym]`\n\
    - ar: `ar=[number]` or `ar[number]`\n\
    - cs: `cs=[number]` or `cs[number]`\n\
    - hp: `hp=[number]` or `hp[number]`\n\
    - od: `od=[number]` or `od[number]`"
)]
#[usage(
    "[map url / map id] [+mods] [acc%] [combox] [clockrate*] \
    [n300x300] [n100x100] [n50x50] [missesm] [katusxkatus]"
)]
#[example("1980365 +hdhr 4000x 1m 2499x300 99.1% 1.05*")]
#[alias("sc", "simc", "simctb", "simcatch", "simulatecatch")]
#[group(Catch)]
async fn prefix_simulatectb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let orig = CommandOrigin::from(msg);

    match Simulate::from_args(Some(GameMode::Catch), args) {
        Ok(args) => simulate(ctx, orig, args).await,
        Err(content) => orig.error(&ctx, content).await,
    }
}

#[command]
#[desc("Simulate a mania score on a map")]
#[help(
    "Simulate a mania score on the given map.\n\
    If no map is specified by either url or id, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Various arguments can be specified in multiple ways:\n\
    - Accuracy: `acc=[number]` or `[number]%`\n\
    - Combo: `combo=[integer]` or `[integer]x`\n\
    - Clock rate: `clockrate=[number]` or `[number]*`\n\
    - n320: `n320=[integer]` or `[integer]x320`\n\
    - n300: `n300=[integer]` or `[integer]x300`\n\
    - n200: `n200=[integer]` or `[integer]x200`\n\
    - n100: `n100=[interger]` or `[integer]x100`\n\
    - n50: `n50=[integer]` or `[integer]x50`\n\
    - misses: `miss=[integer]` or `[integer]m`\n\
    - mods: `mods=[mod acronym]` or `+[mod acronym]`\n\
    - ar: `ar=[number]` or `ar[number]`\n\
    - cs: `cs=[number]` or `cs[number]`\n\
    - hp: `hp=[number]` or `hp[number]`\n\
    - od: `od=[number]` or `od[number]`"
)]
#[usage(
    "[map url / map id] [+mods] [acc%] [combox] [clockrate*] \
    [n300x300] [n100x100] [n50x50] [missesm] [n320x320] [n200x200]"
)]
#[example("1980365 +hdhr 1m 4000x 2499x300 99.1% 1.05* 42x200")]
#[alias("sm", "simm", "simmania")]
#[group(Mania)]
async fn prefix_simulatemania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let orig = CommandOrigin::from(msg);

    match Simulate::from_args(Some(GameMode::Mania), args) {
        Ok(args) => simulate(ctx, orig, args).await,
        Err(content) => orig.error(&ctx, content).await,
    }
}

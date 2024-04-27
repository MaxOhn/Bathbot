pub mod args;
pub mod parsed_map;

use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_util::{constants::GENERAL_ISSUE, matcher, osu::MapIdType};
use eyre::Result;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{message::MessageType, Attachment, Message},
    guild::Permissions,
};

use self::args::{ParseError, SimulateArg};
use super::{
    HasMods, ModsResult, TopOldCatchVersion, TopOldManiaVersion, TopOldOsuVersion,
    TopOldTaikoVersion,
};
use crate::{
    active::{
        impls::{SimulateAttributes, SimulateComponents, SimulateData, SimulateMap, TopOldVersion},
        ActiveMessages,
    },
    commands::{osu::parsed_map::AttachedSimulateMap, GameModeOption},
    core::{
        commands::{prefix::Args, CommandOrigin},
        Context, ContextExt,
    },
    manager::MapError,
    util::{interaction::InteractionCommand, CheckPermissions, InteractionCommandExt},
};

#[derive(CreateCommand, CommandModel, Default, HasMods, SlashCommand)]
#[command(name = "simulate", desc = "Simulate a score on a map")]
pub struct Simulate<'m> {
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<Cow<'m, str>>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify mods")]
    mods: Option<Cow<'m, str>>,
    #[command(desc = "Specify a combo")]
    combo: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0, desc = "Specify an accuracy")]
    acc: Option<f32>,
    #[command(desc = "Specify a custom clock rate that overwrites mods")]
    clock_rate: Option<f32>,
    #[command(desc = "Specify a BPM value instead of a clock rate")]
    bpm: Option<f32>,
    #[command(desc = "Specify the amount of 300s")]
    n300: Option<u32>,
    #[command(desc = "Specify the amount of 100s")]
    n100: Option<u32>,
    #[command(desc = "Specify the amount of 50s")]
    n50: Option<u32>,
    #[command(desc = "Specify misses")]
    misses: Option<u32>,
    #[command(desc = "Specify gekis i.e. n320 in mania")]
    geki: Option<u32>,
    #[command(desc = "Specify katus i.e. tiny droplet misses in catch and n200 in mania")]
    katu: Option<u32>,
    #[command(desc = "Overwrite the map's approach rate")]
    ar: Option<f32>,
    #[command(desc = "Overwrite the map's circle size")]
    cs: Option<f32>,
    #[command(desc = "Overwrite the map's drain rate")]
    hp: Option<f32>,
    #[command(desc = "Overwrite the map's overall difficulty")]
    od: Option<f32>,
    #[command(desc = "Specify a .osu file")]
    file: Option<Attachment>,
}

pub async fn slash_simulate(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Simulate::from_interaction(command.input_data())?;
    let orig = CommandOrigin::from(&mut command);

    match SimulateArgs::from_simulate(args) {
        Ok(args) => simulate(ctx, orig, args).await,
        Err(content) => orig.error(&ctx, content).await,
    }
}

async fn simulate(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mut args: SimulateArgs,
) -> Result<()> {
    let map = args.map.take();
    let mode = args.mode;

    let Some(map) = prepare_map(ctx.cloned(), &orig, map, mode).await? else {
        return Ok(());
    };

    let mode = map.mode();

    let version = match mode {
        GameMode::Osu => TopOldVersion::Osu(TopOldOsuVersion::September22Now),
        GameMode::Taiko => TopOldVersion::Taiko(TopOldTaikoVersion::September22Now),
        GameMode::Catch => TopOldVersion::Catch(TopOldCatchVersion::May20Now),
        GameMode::Mania => TopOldVersion::Mania(TopOldManiaVersion::October22Now),
    };

    let max_combo = match map {
        SimulateMap::Full(ref map) => ctx.pp(map).difficulty().await.max_combo(),
        SimulateMap::Attached(ref map) => map.max_combo,
    };

    let mods = match args.mods.map(|mods| mods.with_mode(mode)) {
        Some(mods @ Some(_)) => mods,
        None => None,
        Some(None) => {
            let content = format!("Looks like those mods are invalid for the {mode:?} mode");

            return orig.error(&ctx, content).await;
        }
    };

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
        bpm: args.bpm,
        attrs: SimulateAttributes {
            ar: args.ar,
            cs: args.cs,
            hp: args.hp,
            od: args.od,
        },
        original_attrs: SimulateAttributes::from(map.pp_map()),
        score: None,
        version,
        max_combo,
    };

    let active = SimulateComponents::new(map, simulate_data, orig.user_id()?);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(ctx, orig)
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
    - Clock rate: `clockrate=[number]` or `[number]*` or `rate=[number]`\n\
    - Bpm: `bpm=[number]` (only if clock rate is not specified)\n\
    - n300: `n300=[integer]` or `[integer]x300`\n\
    - n100: `n100=[integer]` or `[integer]x100`\n\
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
async fn prefix_simulate(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(&ctx, None, msg, args).await {
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
    - Clock rate: `clockrate=[number]` or `[number]*` or `rate=[number]`\n\
    - Bpm: `bpm=[number]` (only if clock rate is not specified)\n\
    - n300: `n300=[integer]` or `[integer]x300`\n\
    - n100: `n100=[integer]` or `[integer]x100`\n\
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
async fn prefix_simulatetaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(&ctx, Some(GameMode::Taiko), msg, args).await {
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
    - Clock rate: `clockrate=[number]` or `[number]*` or `rate=[number]`\n\
    - Bpm: `bpm=[number]` (only if clock rate is not specified)\n\
    - fruits: `n300=[integer]` or `[integer]x300`\n\
    - droplets: `n100=[integer]` or `[integer]x100`\n\
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
async fn prefix_simulatectb(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(&ctx, Some(GameMode::Catch), msg, args).await {
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
    - Clock rate: `clockrate=[number]` or `[number]*` or `rate=[number]`\n\
    - Bpm: `bpm=[number]` (only if clock rate is not specified)\n\
    - n320: `n320=[integer]` or `[integer]x320`\n\
    - n300: `n300=[integer]` or `[integer]x300`\n\
    - n200: `n200=[integer]` or `[integer]x200`\n\
    - n100: `n100=[integer]` or `[integer]x100`\n\
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
async fn prefix_simulatemania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(&ctx, Some(GameMode::Mania), msg, args).await {
        Ok(args) => simulate(ctx, orig, args).await,
        Err(content) => orig.error(&ctx, content).await,
    }
}

async fn prepare_map(
    ctx: Arc<Context>,
    orig: &CommandOrigin<'_>,
    map: Option<SimulateMapArg>,
    mode: Option<GameMode>,
) -> Result<Option<SimulateMap>> {
    let map_id = match map {
        Some(SimulateMapArg::Id(MapIdType::Map(id))) => id,
        Some(SimulateMapArg::Id(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(&ctx, content).await.map(|_| None);
        }
        Some(SimulateMapArg::Attachment(attachment)) => {
            return AttachedSimulateMap::new(&ctx, orig, attachment, mode)
                .await
                .map(|opt| opt.map(SimulateMap::Attached))
        }
        None if orig.can_read_history() => {
            let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("Failed to retrieve channel history"));
                }
            };

            match ctx.find_map_id_in_msgs(&msgs, 0).await {
                Some(MapIdType::Map(id)) => id,
                None | Some(MapIdType::Set(_)) => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return orig.error(&ctx, content).await.map(|_| None);
                }
            }
        }
        None => {
            let content =
                "No beatmap specified and lacking permission to search the channel history for maps.\n\
                Try specifying a map either by url to the map, or just by map id, \
                or give me the \"Read Message History\" permission.";

            return orig.error(&ctx, content).await.map(|_| None);
        }
    };

    let map = match ctx.osu_map().map(map_id, None).await {
        Ok(map) => match mode {
            Some(mode) => map.convert(mode),
            None => map,
        },
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?"
            );

            return orig.error(&ctx, content).await.map(|_| None);
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    Ok(Some(SimulateMap::Full(map)))
}

enum SimulateMapArg {
    Id(MapIdType),
    Attachment(Box<Attachment>),
}

#[derive(Default)]
struct SimulateArgs {
    map: Option<SimulateMapArg>,
    mode: Option<GameMode>,
    mods: Option<GameModsIntermode>,
    combo: Option<u32>,
    acc: Option<f32>,
    bpm: Option<f32>,
    clock_rate: Option<f32>,
    n300: Option<u32>,
    n100: Option<u32>,
    n50: Option<u32>,
    misses: Option<u32>,
    geki: Option<u32>,
    katu: Option<u32>,
    ar: Option<f32>,
    cs: Option<f32>,
    hp: Option<f32>,
    od: Option<f32>,
}

impl SimulateArgs {
    async fn from_args(
        ctx: &Context,
        mode: Option<GameMode>,
        msg: &Message,
        args: Args<'_>,
    ) -> Result<Self, Cow<'static, str>> {
        let reply = msg
            .referenced_message
            .as_deref()
            .filter(|_| msg.kind == MessageType::Reply);

        let mut map = None;

        if let Some(reply) = reply {
            if let Some(id) = ctx.find_map_id_in_msg(reply).await {
                map = Some(SimulateMapArg::Id(id));
            }
        }

        let mut simulate = Self {
            mode,
            map,
            ..Default::default()
        };

        for arg in args {
            let id_opt = matcher::get_osu_map_id(arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set));

            if let Some(id) = id_opt {
                simulate.map = Some(SimulateMapArg::Id(id));

                continue;
            }

            match SimulateArg::parse(arg).map_err(ParseError::into_str)? {
                SimulateArg::Acc(val) => simulate.acc = Some(val.clamp(0.0, 100.0)),
                SimulateArg::Bpm(val) => simulate.bpm = Some(val),
                SimulateArg::Combo(val) => simulate.combo = Some(val),
                SimulateArg::ClockRate(val) => simulate.clock_rate = Some(val),
                SimulateArg::N300(val) => simulate.n300 = Some(val),
                SimulateArg::N100(val) => simulate.n100 = Some(val),
                SimulateArg::N50(val) => simulate.n50 = Some(val),
                SimulateArg::Geki(val) => simulate.geki = Some(val),
                SimulateArg::Katu(val) => simulate.katu = Some(val),
                SimulateArg::Miss(val) => simulate.misses = Some(val),
                SimulateArg::Mods(val) => simulate.mods = Some(val),
                SimulateArg::Ar(val) => simulate.ar = Some(val),
                SimulateArg::Cs(val) => simulate.cs = Some(val),
                SimulateArg::Hp(val) => simulate.hp = Some(val),
                SimulateArg::Od(val) => simulate.od = Some(val),
            }
        }

        Ok(simulate)
    }

    fn from_simulate(simulate: Simulate<'_>) -> Result<Self, &'static str> {
        let mods = match simulate.mods() {
            ModsResult::Mods(mods) => Some(mods.into_mods()),
            ModsResult::None => None,
            ModsResult::Invalid => {
                let content = "Failed to parse mods. Be sure to either specify them directly \
                    or through the `+mods` / `+mods!` syntax e.g. `hdhr` or `+hdhr!`";

                return Err(content);
            }
        };

        let mode = simulate.mode.map(GameMode::from);

        let map = match simulate.file {
            Some(attachment) => Some(SimulateMapArg::Attachment(Box::new(attachment))),
            None => match simulate.map {
                Some(map) => matcher::get_osu_map_id(&map)
                    .map(MapIdType::Map)
                    .or_else(|| matcher::get_osu_mapset_id(&map).map(MapIdType::Set))
                    .ok_or(
                        "Failed to parse map url. \
                        Be sure you specify a valid map id or url to a map.",
                    )
                    .map(SimulateMapArg::Id)
                    .map(Some)?,
                None => None,
            },
        };

        Ok(Self {
            map,
            mode,
            mods,
            combo: simulate.combo,
            acc: simulate.acc,
            bpm: simulate.bpm,
            clock_rate: simulate.clock_rate,
            n300: simulate.n300,
            n100: simulate.n100,
            n50: simulate.n50,
            misses: simulate.misses,
            geki: simulate.geki,
            katu: simulate.katu,
            ar: simulate.ar,
            cs: simulate.cs,
            hp: simulate.hp,
            od: simulate.od,
        })
    }
}

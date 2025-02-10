pub mod args;
pub mod parsed_map;

use std::borrow::Cow;

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_model::command_fields::GameModeOption;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{constants::GENERAL_ISSUE, matcher, osu::MapIdType, CowUtils};
use eyre::Result;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{Attachment, Message},
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
    commands::osu::parsed_map::AttachedSimulateMap,
    core::{
        commands::{prefix::Args, CommandOrigin},
        Context,
    },
    manager::MapError,
    util::{interaction::InteractionCommand, osu::MapOrScore, InteractionCommandExt},
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
    clock_rate: Option<f64>,
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
    #[command(desc = "Whether the score is set on lazer or stable")]
    lazer: Option<bool>,
    #[command(desc = "Specify the amount of slider end hits")]
    slider_end_hits: Option<u32>,
    #[command(desc = "Specify the amount of large tick hits")]
    large_tick_hits: Option<u32>,
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

pub async fn slash_simulate(mut command: InteractionCommand) -> Result<()> {
    let args = Simulate::from_interaction(command.input_data())?;
    let orig = CommandOrigin::from(&mut command);

    match SimulateArgs::from_simulate(args) {
        Ok(args) => simulate(orig, args).await,
        Err(content) => orig.error(content).await,
    }
}

async fn simulate(orig: CommandOrigin<'_>, mut args: SimulateArgs) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let map = args.map.take();
    let mode = args.mode.or(config.mode);

    let Some(map) = prepare_map(&orig, map, mode).await? else {
        return Ok(());
    };

    let mode = map.mode();

    let version = match mode {
        GameMode::Osu => TopOldVersion::Osu(TopOldOsuVersion::October24Now),
        GameMode::Taiko => TopOldVersion::Taiko(TopOldTaikoVersion::October24Now),
        GameMode::Catch => TopOldVersion::Catch(TopOldCatchVersion::October24Now),
        GameMode::Mania => TopOldVersion::Mania(TopOldManiaVersion::October24Now),
    };

    let max_combo = match map {
        SimulateMap::Full(ref map) => Context::pp(map).difficulty().await.max_combo(),
        SimulateMap::Attached(ref map) => map.max_combo,
    };

    let mods = match args.mods.map(|mods| mods.try_with_mode(mode)) {
        Some(mods @ Some(_)) => mods,
        None => None,
        Some(None) => {
            let content = format!("Looks like those mods are invalid for the {mode:?} mode");

            return orig.error(content).await;
        }
    };

    let set_on_lazer = match args.set_on_lazer {
        Some(lazer) => lazer,
        None => !match config.score_data {
            Some(score_data) => score_data.is_legacy(),
            None => match orig.guild_id() {
                Some(guild_id) => Context::guild_config()
                    .peek(guild_id, |config| config.score_data)
                    .await
                    .is_some_and(ScoreData::is_legacy),
                None => false,
            },
        },
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
        set_on_lazer,
        n_slider_ends: args.slider_end_hits,
        n_large_ticks: args.large_tick_hits,
        combo: args.combo,
        clock_rate: args.clock_rate,
        bpm: args.bpm,
        attrs: SimulateAttributes {
            ar: args.ar,
            cs: args.cs,
            hp: args.hp,
            od: args.od,
        },
        score: None,
        version,
        max_combo,
    };

    let active = SimulateComponents::new(map, simulate_data, owner);

    ActiveMessages::builder(active)
        .start_by_update(true)
        .begin(orig)
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
    - slider ends: `sliderends=[integer]` or `[integer]xsliderends`\n\
    - large ticks: `largeticks=[integer]` or `[integer]xlargeticks`\n\
    - small ticks: `smallticks=[integer]` or `[integer]xsmallticks`\n\
    - mods: `mods=[mod acronym]` or `+[mod acronym]`\n\
    - ar: `ar=[number]` or `ar[number]`\n\
    - cs: `cs=[number]` or `cs[number]`\n\
    - hp: `hp=[number]` or `hp[number]`\n\
    - od: `od=[number]` or `od[number]`\n\
    - lazer: `lazer=[bool]` or `stable=[bool]`"
)]
#[usage(
    "[map url / map id] [+mods] [acc%] [combox] [clockrate*] \
    [n300x300] [n100x100] [n50x50] [missesm] [gekisxgeki] [katusxkatus]"
)]
#[example("1980365 +hdhr 4000x 1m 2499x300 99.1% 1.05*")]
#[alias("s", "sim")]
#[group(Osu)]
async fn prefix_simulate(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(None, msg, args).await {
        Ok(args) => simulate(orig, args).await,
        Err(content) => orig.error(content).await,
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
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(Some(GameMode::Taiko), msg, args).await {
        Ok(args) => simulate(orig, args).await,
        Err(content) => orig.error(content).await,
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
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(Some(GameMode::Catch), msg, args).await {
        Ok(args) => simulate(orig, args).await,
        Err(content) => orig.error(content).await,
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
    - od: `od=[number]` or `od[number]`\n\
    - lazer: `lazer=[bool]` or `stable=[bool]`"
)]
#[usage(
    "[map url / map id] [+mods] [acc%] [combox] [clockrate*] \
    [n300x300] [n100x100] [n50x50] [missesm] [n320x320] [n200x200]"
)]
#[example("1980365 +hdhr 1m 4000x 2499x300 99.1% 1.05* 42x200")]
#[alias("sm", "simm", "simmania")]
#[group(Mania)]
async fn prefix_simulatemania(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let orig = CommandOrigin::from_msg(msg, permissions);

    match SimulateArgs::from_args(Some(GameMode::Mania), msg, args).await {
        Ok(args) => simulate(orig, args).await,
        Err(content) => orig.error(content).await,
    }
}

async fn prepare_map(
    orig: &CommandOrigin<'_>,
    map: Option<SimulateMapArg>,
    mode: Option<GameMode>,
) -> Result<Option<SimulateMap>> {
    let map_id = match map {
        Some(SimulateMapArg::Id(MapIdType::Map(id))) => id,
        Some(SimulateMapArg::Id(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(content).await.map(|_| None);
        }
        Some(SimulateMapArg::Attachment(attachment)) => {
            return AttachedSimulateMap::new(orig, attachment, mode)
                .await
                .map(|opt| opt.map(SimulateMap::Attached))
        }
        None => {
            let msgs = match Context::retrieve_channel_history(orig.channel_id()).await {
                Ok(msgs) => msgs,
                Err(_) => {
                    let content =
                        "No beatmap specified and lacking permission to search the channel \
                        history for maps.\nTry specifying a map either by url to the map, or \
                        just by map id, or give me the \"Read Message History\" permission.";

                    return orig.error(content).await.map(|_| None);
                }
            };

            match Context::find_map_id_in_msgs(&msgs, 0).await {
                Some(MapIdType::Map(id)) => id,
                None | Some(MapIdType::Set(_)) => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return orig.error(content).await.map(|_| None);
                }
            }
        }
    };

    let map = match Context::osu_map().map(map_id, None).await {
        Ok(map) => match mode {
            Some(mode) => map.convert(mode),
            None => map,
        },
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?"
            );

            return orig.error(content).await.map(|_| None);
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

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
    clock_rate: Option<f64>,
    n300: Option<u32>,
    n100: Option<u32>,
    n50: Option<u32>,
    misses: Option<u32>,
    set_on_lazer: Option<bool>,
    slider_end_hits: Option<u32>,
    large_tick_hits: Option<u32>,
    geki: Option<u32>,
    katu: Option<u32>,
    ar: Option<f32>,
    cs: Option<f32>,
    hp: Option<f32>,
    od: Option<f32>,
}

impl SimulateArgs {
    async fn from_args(
        mode: Option<GameMode>,
        msg: &Message,
        args: Args<'_>,
    ) -> Result<Self, Cow<'static, str>> {
        let map = match MapOrScore::find_in_msg(msg).await {
            Some(MapOrScore::Map(id)) => Some(SimulateMapArg::Id(id)),
            Some(MapOrScore::Score { .. }) => {
                return Err(Cow::Borrowed(
                    "This command does not (yet) accept score urls as argument",
                ))
            }
            None => None,
        };

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

            let arg = arg.cow_to_ascii_lowercase();

            match SimulateArg::parse(&arg).map_err(ParseError::into_str)? {
                SimulateArg::Acc(val) => simulate.acc = Some(val.clamp(0.0, 100.0)),
                SimulateArg::Bpm(val) => simulate.bpm = Some(val),
                SimulateArg::Combo(val) => simulate.combo = Some(val),
                SimulateArg::ClockRate(val) => simulate.clock_rate = Some(val as f64),
                SimulateArg::N300(val) => simulate.n300 = Some(val),
                SimulateArg::N100(val) => simulate.n100 = Some(val),
                SimulateArg::N50(val) => simulate.n50 = Some(val),
                SimulateArg::Geki(val) => simulate.geki = Some(val),
                SimulateArg::Katu(val) => simulate.katu = Some(val),
                SimulateArg::Miss(val) => simulate.misses = Some(val),
                SimulateArg::SliderEnds(val) | SimulateArg::SmallTicks(val) => {
                    simulate.slider_end_hits = Some(val)
                }
                SimulateArg::LargeTicks(val) => simulate.large_tick_hits = Some(val),
                SimulateArg::Mods(val) => simulate.mods = Some(val),
                SimulateArg::Ar(val) => simulate.ar = Some(val),
                SimulateArg::Cs(val) => simulate.cs = Some(val),
                SimulateArg::Hp(val) => simulate.hp = Some(val),
                SimulateArg::Od(val) => simulate.od = Some(val),
                SimulateArg::Lazer(val) => simulate.set_on_lazer = Some(val),
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
            set_on_lazer: simulate.lazer,
            slider_end_hits: simulate.slider_end_hits,
            large_tick_hits: simulate.large_tick_hits,
            geki: simulate.geki,
            katu: simulate.katu,
            ar: simulate.ar,
            cs: simulate.cs,
            hp: simulate.hp,
            od: simulate.od,
        })
    }
}

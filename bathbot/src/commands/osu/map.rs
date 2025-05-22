use std::{borrow::Cow, cmp::Ordering, fmt::Write};

use bathbot_macros::{HasMods, SlashCommand, command};
use bathbot_util::{
    MessageOrigin,
    constants::OSU_API_ISSUE,
    matcher,
    osu::{MapIdType, ModSelection},
};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, GameModsIntermode, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{channel::Message, guild::Permissions};

use super::{HasMods, ModsResult};
use crate::{
    Context,
    active::{
        ActiveMessages,
        impls::{MapPagination, SingleScorePagination},
    },
    commands::osu::map_strains_graph,
    core::commands::{CommandOrigin, prefix::Args},
    util::{ChannelExt, InteractionCommandExt, interaction::InteractionCommand, osu::MapOrScore},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "map",
    desc = "Display a bunch of stats about a map(set)",
    help = "Display a bunch of stats about a map(set).\n\
    The values in the map info will be adjusted to mods.\n\
    Since discord does not allow images to be adjusted when editing messages, \
    the strain graph always belongs to the initial map, even after moving to \
    other maps of the set through the pagination buttons."
)]
pub struct Map<'a> {
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
    If none is specified, it will search in the recent channel history \
    and pick the first map it can find."
    )]
    map: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(desc = "Specify an AR value to override the actual one")]
    ar: Option<f64>,
    #[command(desc = "Specify an OD value to override the actual one")]
    od: Option<f64>,
    #[command(desc = "Specify a CS value to override the actual one")]
    cs: Option<f64>,
    #[command(desc = "Specify an HP value to override the actual one")]
    hp: Option<f64>,
}

#[derive(HasMods)]
struct MapArgs<'a> {
    map: Option<MapIdType>,
    mods: Option<Cow<'a, str>>,
    attrs: CustomAttrs,
}

#[derive(Default)]
pub struct CustomAttrs {
    pub ar: Option<f64>,
    pub cs: Option<f64>,
    pub hp: Option<f64>,
    pub od: Option<f64>,
}

impl CustomAttrs {
    fn content(&self) -> Option<String> {
        self.ar.or(self.cs).or(self.hp).or(self.od)?;

        let mut content = "Custom attributes: ".to_owned();
        let mut pushed = false;

        if let Some(ar) = self.ar {
            let _ = write!(content, "`AR: {ar:.2}`");
            pushed = true;
        }

        if let Some(cs) = self.cs {
            if pushed {
                content.push_str(" ~ ");
            }

            let _ = write!(content, "`CS: {cs:.2}`");
            pushed = true;
        }

        if let Some(hp) = self.hp {
            if pushed {
                content.push_str(" ~ ");
            }

            let _ = write!(content, "`HP: {hp:.2}`");
            pushed = true;
        }

        if let Some(od) = self.od {
            if pushed {
                content.push_str(" ~ ");
            }

            let _ = write!(content, "`OD: {od:.2}`");
        }

        Some(content)
    }
}

impl<'m> MapArgs<'m> {
    async fn args(msg: &Message, args: Args<'m>) -> Result<MapArgs<'m>, String> {
        let mut map = None;
        let mut mods = None;

        for arg in args.take(2) {
            if let Some(id) = matcher::get_osu_map_id(arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set))
            {
                map = Some(id);
            } else if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else {
                let content = format!(
                    "Failed to parse `{arg}`.\n\
                    Be sure you specify either a valid map id, map url, or mod combination."
                );

                return Err(content);
            }
        }

        if map.is_none() {
            match MapOrScore::find_in_msg(msg).await {
                Some(MapOrScore::Map(id)) => map = Some(id),
                Some(MapOrScore::Score { .. }) => {
                    return Err(
                        "This command does not (yet) accept score urls as argument".to_owned()
                    );
                }
                None => {}
            }
        }

        Ok(Self {
            map,
            mods,
            attrs: CustomAttrs::default(),
        })
    }
}

impl<'a> TryFrom<Map<'a>> for MapArgs<'a> {
    type Error = &'static str;

    fn try_from(args: Map<'a>) -> Result<Self, Self::Error> {
        let Map {
            map,
            mods,
            ar,
            od,
            cs,
            hp,
        } = args;

        let map = match map.map(|arg| {
            matcher::get_osu_map_id(&arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(&arg).map(MapIdType::Set))
        }) {
            Some(Some(id)) => Some(id),
            Some(None) => {
                let content =
                    "Failed to parse map url. Be sure you specify a valid map id or url to a map.";

                return Err(content);
            }
            None => None,
        };

        let attrs = CustomAttrs { ar, cs, hp, od };

        Ok(Self { map, mods, attrs })
    }
}

#[command]
#[desc("Display a bunch of stats about a map(set)")]
#[help(
    "Display stats about a beatmap. Mods can be specified.\n\
    If no map(set) is specified by either url or id, I will choose the last map \
    I can find in the embeds of this channel.\n\
    If the mapset is specified by id but there is some map with the same id, \
    I will choose the latter."
)]
#[usage("[map(set) url / map(set) id] [+mods]")]
#[examples("2240404 +hddt", "https://osu.ppy.sh/beatmapsets/902425 +hr")]
#[aliases("m", "beatmap", "maps", "beatmaps", "mapinfo")]
#[group(AllModes)]
async fn prefix_map(msg: &Message, args: Args<'_>, permissions: Option<Permissions>) -> Result<()> {
    match MapArgs::args(msg, args).await {
        Ok(args) => map(CommandOrigin::from_msg(msg, permissions), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

async fn slash_map(mut command: InteractionCommand) -> Result<()> {
    let args = Map::from_interaction(command.input_data())?;

    match MapArgs::try_from(args) {
        Ok(args) => map((&mut command).into(), args).await,
        Err(content) => {
            command.error(content).await?;

            Ok(())
        }
    }
}

async fn map(orig: CommandOrigin<'_>, args: MapArgs<'_>) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content =
                "Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.";

            return orig.error(content).await;
        }
    };

    let MapArgs { map, attrs, .. } = args;

    let map_id = if let Some(id) = map {
        id
    } else {
        let msgs = match Context::retrieve_channel_history(orig.channel_id()).await {
            Ok(msgs) => msgs,
            Err(_) => {
                let content = "No beatmap specified and lacking permission to search the channel history \
                    for maps.\nTry specifying a map(set) either by url to the map, \
                    or just by map(set) id, or give me the \"Read Message History\" permission.";

                return orig.error(content).await;
            }
        };

        match Context::find_map_id_in_msgs(&msgs, 0).await {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map(set) either by url to the map, \
                    or just by map(set) id.";

                return orig.error(content).await;
            }
        }
    };

    debug!(?map_id, "Processing map command...");

    let mods = match mods {
        Some(ModSelection::Include(mods) | ModSelection::Exact(mods)) => mods,
        None | Some(ModSelection::Exclude { .. }) => GameModsIntermode::new(),
    };

    let mapset_res = match map_id {
        MapIdType::Map(id) => Context::osu().beatmapset_from_map_id(id).await,
        MapIdType::Set(id) => Context::osu().beatmapset(id).await,
    };

    let mut mapset = match mapset_res {
        Ok(mapset) => mapset,
        Err(OsuError::NotFound) => {
            let content = match map_id {
                MapIdType::Map(id) => format!("Beatmapset of map {id} was not found"),
                MapIdType::Set(id) => format!("Beatmapset with id {id} was not found"),
            };

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get mapset"));
        }
    };

    let mapset_clone = mapset.clone();
    tokio::spawn(async move { Context::osu_map().store(&mapset_clone).await });

    let Some(mut maps) = mapset.maps.take().filter(|maps| !maps.is_empty()) else {
        return orig.error("The mapset has no maps").await;
    };

    maps.sort_unstable_by(|m1, m2| {
        m1.mode.cmp(&m2.mode).then_with(|| match m1.mode {
            // For mania sort first by mania key, then star rating
            GameMode::Mania => m1
                .cs
                .partial_cmp(&m2.cs)
                .unwrap_or(Ordering::Equal)
                .then(m1.stars.partial_cmp(&m2.stars).unwrap_or(Ordering::Equal)),
            // For other mods just sort by star rating
            _ => m1.stars.partial_cmp(&m2.stars).unwrap_or(Ordering::Equal),
        })
    });

    let map_idx = match map_id {
        MapIdType::Map(map_id) => maps
            .iter()
            .position(|map| map.map_id == map_id)
            .unwrap_or(0),
        MapIdType::Set(_) => 0,
    };

    let map_id = maps[map_idx].map_id;
    let mode = maps[map_idx].mode;

    let mods_with_mode = match mods.clone().try_with_mode(mode) {
        Some(mods) if mods.is_valid() => mods,
        Some(_) => {
            let content =
                format!("Looks like some mods in `{mods}` are incompatible with each other");

            return orig.error(content).await;
        }
        None => {
            let content = format!(
                "The mods `{mods}` are incompatible with the map's mode {:?}",
                maps[map_idx].mode
            );

            return orig.error(content).await;
        }
    };

    let graph = match Context::osu_map().pp_map(map_id).await {
        Ok(map) => {
            let w = SingleScorePagination::IMAGE_W;
            let h = SingleScorePagination::IMAGE_H;

            match map_strains_graph(&map, mods_with_mode, &mapset.covers.cover, w, h).await {
                Ok(graph) => Some(graph),
                Err(err) => {
                    warn!(?err, "Failed to create graph");

                    None
                }
            }
        }
        Err(err) => {
            warn!(?err, "Failed to get pp map");

            None
        }
    };

    let content = attrs.content();

    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());

    let mut pagination = MapPagination::builder()
        .mapset(mapset)
        .maps(maps.into_boxed_slice())
        .mods(mods)
        .attrs(attrs)
        .origin(origin)
        .content(content.unwrap_or_default().into_boxed_str())
        .msg_owner(orig.user_id()?)
        .build();

    pagination.set_index(map_idx);

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .attachment(graph.map(|bytes| ("map_graph.png".to_owned(), bytes)))
        .begin(orig)
        .await
}

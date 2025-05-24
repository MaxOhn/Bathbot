use std::{iter, ops::ControlFlow};

use bathbot_macros::{HasMods, HasName, SlashCommand, command};
use bathbot_model::{
    Countries,
    command_fields::{GameModeOption, ShowHideOption, TimezoneOption},
};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    EmbedBuilder, MessageBuilder,
    constants::{GENERAL_ISSUE, OSU_BASE},
    matcher,
    osu::{MapIdType, ModSelection, ModsResult},
};
use eyre::{Report, Result, WrapErr};
use image::{DynamicImage, GenericImageView};
use plotters::element::{Drawable, PointCollection};
use plotters_backend::{BackendCoord, DrawingBackend, DrawingErrorKind};
use plotters_skia::SkiaBackend;
use rosu_v2::{
    prelude::{GameMode, GameMods, OsuError},
    request::UserId,
};
use time::UtcOffset;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{
    Id,
    marker::{ChannelMarker, UserMarker},
};

pub use self::map_strains::map_strains_graph;
use self::{
    bpm::map_bpm_graph,
    medals::medals_graph,
    playcount_replays::{ProfileGraphFlags, playcount_replays_graph},
    rank::rank_graph,
    score_rank::score_rank_graph,
    snipe_count::snipe_count_graph,
    sniped::sniped_graph,
    top_date::top_graph_date,
    top_index::top_graph_index,
    top_time::{top_graph_time_day, top_graph_time_hour},
};
use super::{SnipeGameMode, require_link, user_not_found};
use crate::{
    commands::osu::HasMods,
    core::{Context, commands::CommandOrigin},
    embeds::attachment,
    manager::{
        MapError, OsuMap,
        redis::osu::{CachedUser, UserArgs, UserArgsError},
    },
    util::{CachedUserExt, InteractionCommandExt, interaction::InteractionCommand},
};

mod bpm;
mod map_strains;
mod medals;
mod playcount_replays;
mod rank;
mod score_rank;
mod snipe_count;
mod sniped;
mod top_date;
mod top_index;
mod top_time;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "graph", desc = "Display graphs about some user data")]
pub enum Graph {
    #[command(name = "bpm")]
    MapBpm(GraphMapBpm),
    #[command(name = "strains")]
    MapStrains(GraphMapStrains),
    #[command(name = "medals")]
    Medals(GraphMedals),
    #[command(name = "playcount_replays")]
    PlaycountReplays(GraphPlaycountReplays),
    #[command(name = "rank")]
    Rank(GraphRank),
    #[command(name = "score_rank")]
    ScoreRank(GraphScoreRank),
    #[command(name = "sniped")]
    Sniped(GraphSniped),
    #[command(name = "snipe_count")]
    SnipeCount(GraphSnipeCount),
    #[command(name = "top")]
    Top(GraphTop),
}

#[derive(CommandModel, CreateCommand, HasMods)]
#[command(name = "bpm", desc = "Display a map's bpm over time")]
pub struct GraphMapBpm {
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<String>,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Option<String>,
}

#[derive(CommandModel, CreateCommand, HasMods)]
#[command(name = "strains", desc = "Display a map's strains over time")]
pub struct GraphMapStrains {
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<String>,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Option<String>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "medals", desc = "Display a user's medal progress over time")]
pub struct GraphMedals {
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "playcount_replays",
    desc = "Display a user's playcount and replays watched over time"
)]
pub struct GraphPlaycountReplays {
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(desc = "Specify if the playcount curve should be included")]
    playcount: Option<ShowHideOption>,
    #[command(desc = "Specify if the replay curve should be included")]
    replays: Option<ShowHideOption>,
    #[command(desc = "Specify if the badges should be included")]
    badges: Option<ShowHideOption>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "rank", desc = "Display a user's rank progression over time")]
pub struct GraphRank {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(desc = "From this many days ago", min_value = 0, max_value = 90)]
    from: Option<u8>,
    #[command(desc = "Until this many days ago", min_value = 0, max_value = 90)]
    until: Option<u8>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "score_rank",
    desc = "Display a user's score rank progression over time"
)]
pub struct GraphScoreRank {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "sniped", desc = "Display sniped users of the past 8 weeks")]
pub struct GraphSniped {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "snipe_count",
    desc = "Display how a user's national #1 count progressed"
)]
pub struct GraphSnipeCount {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "top",
    desc = "Display a user's top scores pp",
    help = "Display a user's top scores pp.\n\
    The timezone option is only relevant for the `Time` order."
)]
pub struct GraphTop {
    #[command(desc = "Choose by which order the scores should be sorted, defaults to index")]
    order: GraphTopOrder,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(desc = "Specify a timezone (only relevant when ordered by `Time`)")]
    timezone: Option<TimezoneOption>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandOption, CreateOption)]
pub enum GraphTopOrder {
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Index", value = "index")]
    Index,
    #[option(name = "Time by hour", value = "time_h")]
    TimeByHour,
    #[option(name = "Time by day", value = "time_d")]
    TimeByDay,
}

async fn slash_graph(mut command: InteractionCommand) -> Result<()> {
    let args = Graph::from_interaction(command.input_data())?;

    graph((&mut command).into(), args).await
}

// Takes a `CommandOrigin` since `require_link` does not take
// `InteractionCommand`
async fn graph(orig: CommandOrigin<'_>, args: Graph) -> Result<()> {
    let tuple_option = match args {
        Graph::MapBpm(args) => {
            return match map_bpm(&orig, args).await {
                Ok(ControlFlow::Continue(map)) => {
                    orig.create_message(map.into()).await?;

                    Ok(())
                }
                Ok(ControlFlow::Break(())) => Ok(()),
                Err(err) => Err(err.wrap_err("Failed to create map bpm graph")),
            };
        }
        Graph::MapStrains(args) => {
            return match map_strains(&orig, args).await {
                Ok(ControlFlow::Continue(map)) => {
                    orig.create_message(map.into()).await?;

                    Ok(())
                }
                Ok(ControlFlow::Break(())) => Ok(()),
                Err(err) => Err(err.wrap_err("Failed to create map strains graph")),
            };
        }
        Graph::Medals(args) => {
            let user_id = match user_id!(orig, args) {
                Some(user_id) => user_id,
                None => match Context::user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&orig).await,
                    Err(err) => {
                        let _ = orig.error(GENERAL_ISSUE).await;

                        return Err(err);
                    }
                },
            };

            medals_graph(&orig, user_id)
                .await
                .wrap_err("failed to create medals graph")?
        }
        Graph::PlaycountReplays(args) => {
            let user_id = match user_id!(orig, args) {
                Some(user_id) => user_id,
                None => match Context::user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&orig).await,
                    Err(err) => {
                        let _ = orig.error(GENERAL_ISSUE).await;

                        return Err(err);
                    }
                },
            };

            let mut flags = ProfileGraphFlags::all();

            if let Some(ShowHideOption::Hide) = args.playcount {
                flags &= !ProfileGraphFlags::PLAYCOUNT;
            }

            if let Some(ShowHideOption::Hide) = args.replays {
                flags &= !ProfileGraphFlags::REPLAYS;
            }

            if let Some(ShowHideOption::Hide) = args.badges {
                flags &= !ProfileGraphFlags::BADGES;
            }

            if flags.is_empty() {
                return orig.error(":clown:").await;
            }

            playcount_replays_graph(&orig, user_id, flags)
                .await
                .wrap_err("failed to create profile graph")?
        }
        Graph::Rank(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);
            let user_args = UserArgs::rosu_id(&user_id, mode).await;

            rank_graph(&orig, user_id, user_args, args.from, args.until)
                .await
                .wrap_err("Failed to create rank graph")?
        }
        Graph::ScoreRank(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);

            let tuple_option = score_rank_graph(&orig, user_id, mode)
                .await
                .wrap_err("Failed to create score rank graph")?;

            let Some((author, graph)) = tuple_option else {
                return Ok(());
            };

            let embed = EmbedBuilder::new()
                .author(author)
                .image(attachment("graph.png"));

            let builder = MessageBuilder::new()
                .embed(embed)
                .attachment("graph.png", graph);

            orig.create_message(builder).await?;

            return Ok(());
        }
        Graph::Sniped(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);

            sniped_graph(&orig, user_id, mode)
                .await
                .wrap_err("failed to create snipe graph")?
        }
        Graph::SnipeCount(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);

            snipe_count_graph(&orig, user_id, mode)
                .await
                .wrap_err("failed to create snipe count graph")?
        }
        Graph::Top(args) => {
            let owner = orig.user_id()?;

            let config = match Context::user_config().with_osu_id(owner).await {
                Ok(config) => config,
                Err(err) => {
                    let _ = orig.error(GENERAL_ISSUE).await;

                    return Err(err.wrap_err("failed to get user config"));
                }
            };

            let mode = args
                .mode
                .map(GameMode::from)
                .or(config.mode)
                .unwrap_or(GameMode::Osu);

            let (user_id, no_user_specified) = match user_id!(orig, args) {
                Some(user_id) => (user_id, false),
                None => match config.osu {
                    Some(user_id) => (UserId::Id(user_id), true),
                    None => return require_link(&orig).await,
                },
            };

            let user_args = UserArgs::rosu_id(&user_id, mode).await;

            let tz = args
                .timezone
                .map(UtcOffset::from)
                .or_else(|| no_user_specified.then_some(config.timezone).flatten());

            let legacy_scores = match config.score_data {
                Some(score_data) => score_data.is_legacy(),
                None => match orig.guild_id() {
                    Some(guild_id) => Context::guild_config()
                        .peek(guild_id, |config| config.score_data)
                        .await
                        .is_some_and(ScoreData::is_legacy),
                    None => false,
                },
            };

            top_graph(&orig, user_id, user_args, args.order, tz, legacy_scores)
                .await
                .wrap_err("failed to create top graph")?
        }
    };

    let Some((user, graph)) = tuple_option else {
        return Ok(());
    };

    let embed = EmbedBuilder::new()
        .author(user.author_builder(false))
        .image(attachment("graph.png"));

    let builder = MessageBuilder::new()
        .embed(embed)
        .attachment("graph.png", graph);

    orig.create_message(builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 711;

struct MapResult {
    bytes: Vec<u8>,
    title: String,
    url: String,
}

impl MapResult {
    fn new(map: &OsuMap, bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            title: format!("{} - {} [{}]", map.artist(), map.title(), map.version()),
            url: format!("{OSU_BASE}b/{}", map.map_id()),
        }
    }
}

impl From<MapResult> for MessageBuilder<'_> {
    fn from(map: MapResult) -> Self {
        let embed = EmbedBuilder::new()
            .image(attachment("graph.png"))
            .title(map.title)
            .url(map.url);

        Self::new().embed(embed).attachment("graph.png", map.bytes)
    }
}

async fn get_map_id(map: Option<&str>, channel_id: Id<ChannelMarker>) -> Result<u32, &'static str> {
    let map = match map.map(|arg| {
        matcher::get_osu_map_id(arg)
            .map(MapIdType::Map)
            .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set))
    }) {
        Some(Some(id)) => Some(id),
        Some(None) => {
            return Err(
                "Failed to parse map url. Be sure you specify a valid map id or url to a map.",
            );
        }
        None => None,
    };

    let map_id = if let Some(id) = map {
        id
    } else {
        let Ok(msgs) = Context::retrieve_channel_history(channel_id).await else {
            return Err(
                "No beatmap specified and lacking permission to search the channel history for \
                maps.\nTry specifying a map either by url or by map id, or give me the \"Read \
                Message History\" permission.",
            );
        };

        match Context::find_map_id_in_msgs(&msgs, 0).await {
            Some(id) => id,
            None => {
                return Err(
                    "No beatmap specified and none found in recent channel history. Try \
                    specifying a map either by url or by map id.",
                );
            }
        }
    };

    let MapIdType::Map(map_id) = map_id else {
        return Err("Looks like you gave me a mapset id, I need a map id though");
    };

    Ok(map_id)
}

async fn map_bpm(
    orig: &CommandOrigin<'_>,
    args: GraphMapBpm,
) -> Result<ControlFlow<(), MapResult>> {
    let mods_res = args.mods();

    let map_id = match get_map_id(args.map.as_deref(), orig.channel_id()).await {
        Ok(map_id) => map_id,
        Err(content) => return orig.error(content).await.map(ControlFlow::Break),
    };

    let map = match Context::osu_map().map(map_id, None).await {
        Ok(map) => map,
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?",
            );

            return orig.error(content).await.map(ControlFlow::Break);
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mods = match mods_res {
        ModsResult::Mods(ModSelection::Include(mods) | ModSelection::Exact(mods)) => {
            let opt = [
                GameMode::Osu,
                GameMode::Taiko,
                GameMode::Catch,
                GameMode::Mania,
            ]
            .into_iter()
            .filter_map(|mode| mods.clone().try_with_mode(mode))
            .find(GameMods::is_valid);

            match opt {
                Some(mods) => mods,
                None => {
                    let content = format!(
                        "Looks like either some mods in `{mods}` are incompatible with each other \
                        or do not belong to any single mode"
                    );

                    return orig.error(content).await.map(ControlFlow::Break);
                }
            }
        }
        ModsResult::Mods(ModSelection::Exclude { .. }) | ModsResult::None => GameMods::new(),
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(content).await.map(ControlFlow::Break);
        }
    };

    let bytes = map_bpm_graph(&map.pp_map, mods, map.cover()).await?;

    Ok(ControlFlow::Continue(MapResult::new(&map, bytes)))
}

async fn map_strains(
    orig: &CommandOrigin<'_>,
    args: GraphMapStrains,
) -> Result<ControlFlow<(), MapResult>> {
    let mods_res = args.mods();

    let map_id = match get_map_id(args.map.as_deref(), orig.channel_id()).await {
        Ok(map_id) => map_id,
        Err(content) => return orig.error(content).await.map(ControlFlow::Break),
    };

    let mode = args.mode.map(GameMode::from);

    let map = match Context::osu_map().map(map_id, None).await {
        Ok(mut map) => {
            if let Some(mode) = mode {
                map.convert_mut(mode);
            }

            map
        }
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                        Did you give me a mapset id instead of a map id?",
            );

            return orig.error(content).await.map(ControlFlow::Break);
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = mode.unwrap_or(map.mode());

    let mods = match mods_res {
        ModsResult::Mods(ModSelection::Include(mods) | ModSelection::Exact(mods)) => {
            match mods.try_with_mode(mode) {
                Some(mods) if mods.is_valid() => mods,
                Some(_) => {
                    let content = format!(
                        "Looks like some mods in `{mods}` are incompatible with each other"
                    );

                    return orig.error(content).await.map(ControlFlow::Break);
                }
                None => {
                    let content =
                        format!("The mods `{mods}` are incompatible with the mode {mode:?}");

                    return orig.error(content).await.map(ControlFlow::Break);
                }
            }
        }
        ModsResult::Mods(ModSelection::Exclude { .. }) | ModsResult::None => GameMods::new(),
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(content).await.map(ControlFlow::Break);
        }
    };

    let bytes = map_strains_graph(&map.pp_map, mods, map.cover(), W, H).await?;

    Ok(ControlFlow::Continue(MapResult::new(&map, bytes)))
}

async fn top_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
    user_args: UserArgs,
    order: GraphTopOrder,
    tz: Option<UtcOffset>,
    legacy_scores: bool,
) -> Result<Option<(CachedUser, Vec<u8>)>> {
    let scores_fut = Context::osu_scores()
        .top(200, legacy_scores)
        .exec_with_user(user_args);

    let (user, mut scores) = match scores_fut.await {
        Ok(tuple) => tuple,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;
            orig.error(content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or scores");

            return Err(err);
        }
    };

    if scores.is_empty() {
        let content = "User's top scores are empty";
        orig.error(content).await?;

        return Ok(None);
    }

    let username = user.username.as_str();
    let country_code = user.country_code.as_str();
    let mode = user.mode;

    let caption = format!(
        "{username}'{genitive} {mode}top200",
        genitive = if username.ends_with('s') { "" } else { "s" },
        mode = match mode {
            GameMode::Osu => "",
            GameMode::Taiko => "taiko ",
            GameMode::Catch => "ctb ",
            GameMode::Mania => "mania ",
        }
    );

    let tz = tz.unwrap_or_else(|| Countries::code(country_code).to_timezone());

    let graph_result = match order {
        GraphTopOrder::Date => top_graph_date(caption, &mut scores)
            .await
            .wrap_err("Failed to create top date graph"),
        GraphTopOrder::Index => top_graph_index(caption, &scores)
            .await
            .wrap_err("Failed to create top index graph"),
        GraphTopOrder::TimeByHour => top_graph_time_hour(caption, &mut scores, tz)
            .await
            .wrap_err("Failed to create top time hour graph"),
        GraphTopOrder::TimeByDay => top_graph_time_day(caption, &mut scores, tz)
            .await
            .wrap_err("Failed to create top time day graph"),
    };

    let bytes = match graph_result {
        Ok(graph) => graph,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            warn!("{err:?}");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn get_map_cover(url: &str, w: u32, h: u32) -> Result<DynamicImage> {
    let bytes = Context::client().get_mapset_cover(url).await?;

    let cover =
        image::load_from_memory(&bytes).wrap_err("Failed to load mapset cover from memory")?;

    Ok(cover.thumbnail_exact(w, h))
}

pub struct BitMapElement<C> {
    img: Vec<u8>,
    size: (u32, u32),
    pos: C,
}

impl<C> BitMapElement<C> {
    /// Be sure the image has been resized beforehand
    pub fn new(img: DynamicImage, pos: C) -> Self {
        let size = img.dimensions();
        let img = img.into_rgba8().into_raw();

        Self { img, size, pos }
    }
}

impl<'a, C> PointCollection<'a, C> for &'a BitMapElement<C> {
    type IntoIter = iter::Once<&'a C>;
    type Point = &'a C;

    #[inline]
    fn point_iter(self) -> Self::IntoIter {
        iter::once(&self.pos)
    }
}

impl<'a, C> Drawable<SkiaBackend<'a>> for BitMapElement<C> {
    #[inline]
    fn draw<I: Iterator<Item = BackendCoord>>(
        &self,
        mut points: I,
        backend: &mut SkiaBackend<'a>,
        _: (u32, u32),
    ) -> Result<(), DrawingErrorKind<<SkiaBackend<'_> as DrawingBackend>::ErrorType>> {
        if let Some((x, y)) = points.next() {
            return backend.blit_bitmap((x, y), self.size, self.img.as_ref());
        }

        Ok(())
    }
}

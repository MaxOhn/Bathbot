use std::{borrow::Cow, cmp::Ordering, fmt::Write, iter, sync::Arc, time::Duration};

use command_macros::{command, HasMods, SlashCommand};
use enterpolation::{linear::Linear, Curve};
use eyre::Report;
use image::{
    codecs::png::PngEncoder, ColorType, DynamicImage, GenericImageView, ImageEncoder, Luma, Pixel,
};
use plotters::{
    element::{Drawable, PointCollection},
    prelude::*,
};
use plotters_backend::{BackendColor, BackendCoord, BackendStyle, DrawingErrorKind};
use rosu_pp::{Beatmap, BeatmapExt};
use rosu_v2::prelude::{GameMode, GameMods, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::{message::MessageType, Message},
};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, MapEmbed},
    error::{GraphError, PpError},
    pagination::{MapPagination, Pagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher,
        osu::{prepare_beatmap_file, MapIdType},
        ApplicationCommandExt, ChannelExt,
    },
    BotResult, Context, Error,
};

use super::{HasMods, ModsResult};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "map",
    help = "Display a bunch of stats about a map(set).\n\
The values in the map info will be adjusted to mods.\n\
Since discord does not allow images to be adjusted when editing messages, \
the strain graph always belongs to the initial map, even after moving to \
other maps of the set through the arrow reactions."
)]
/// Display a bunch of stats about a map(set)
pub struct Map<'a> {
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: Option<Cow<'a, str>>,
    #[command(min_value = 0.0, max_value = 10.0)]
    /// Specify an AR value to override the actual one
    ar: Option<f64>,
    #[command(min_value = 0.0, max_value = 10.0)]
    /// Specify an OD value to override the actual one
    od: Option<f64>,
    #[command(min_value = 0.0, max_value = 10.0)]
    /// Specify a CS value to override the actual one
    cs: Option<f64>,
    #[command(min_value = 0.0, max_value = 10.0)]
    /// Specify an HP value to override the actual one
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
    fn args(msg: &Message, args: Args<'m>) -> Result<Self, String> {
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

        let reply = msg
            .referenced_message
            .as_deref()
            .filter(|_| msg.kind == MessageType::Reply);

        if let Some(id) = reply.and_then(MapIdType::from_msg) {
            map = Some(id);
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
async fn prefix_map(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match MapArgs::args(msg, args) {
        Ok(args) => map(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_map(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Map::from_interaction(command.input_data())?;

    match MapArgs::try_from(args) {
        Ok(args) => map(ctx, command.into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

const W: u32 = 590;
const H: u32 = 150;

async fn map(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: MapArgs<'_>) -> BotResult<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content =
                "Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.";

            return orig.error(&ctx, content).await;
        }
    };

    let MapArgs { map, attrs, .. } = args;
    let author_id = orig.user_id()?;

    let map_id = if let Some(id) = map {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
            Ok(msgs) => msgs,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        };

        match MapIdType::from_msgs(&msgs) {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map(set) either by url to the map, \
                    or just by map(set) id.";

                return orig.error(&ctx, content).await;
            }
        }
    };

    let mods = match mods {
        Some(selection) => selection.mods(),
        None => GameMods::NoMod,
    };

    // Retrieving the beatmaps
    let (mapset_id, map_id) = match map_id {
        // If its given as map id, try to convert into mapset id
        MapIdType::Map(id) => {
            // Check if map is in DB
            match ctx.psql().get_beatmap(id, false).await {
                Ok(map) => (map.mapset_id, Some(id)),
                Err(_) => {
                    // If not in DB, request through API
                    match ctx.osu().beatmap().map_id(id).await {
                        Ok(map) => {
                            // Store map in DB
                            if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                                warn!("{:?}", Report::new(err));
                            }

                            (map.mapset_id, Some(id))
                        }
                        Err(OsuError::NotFound) => (id, None),
                        Err(err) => {
                            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                            return Err(err.into());
                        }
                    }
                }
            }
        }

        // If its already given as mapset id, do nothing
        MapIdType::Set(id) => (id, None),
    };

    // Request mapset through API for all maps + genre & language
    let (mapset, maps) = match ctx.osu().beatmapset(mapset_id).await {
        Ok(mut mapset) => {
            let mut maps = mapset.maps.take().unwrap_or_default();

            maps.sort_unstable_by(|m1, m2| {
                (m1.mode as u8)
                    .cmp(&(m2.mode as u8))
                    .then_with(|| match m1.mode {
                        // For mania sort first by mania key, then star rating
                        GameMode::MNA => m1
                            .cs
                            .partial_cmp(&m2.cs)
                            .unwrap_or(Ordering::Equal)
                            .then(m1.stars.partial_cmp(&m2.stars).unwrap_or(Ordering::Equal)),
                        // For other mods just sort by star rating
                        _ => m1.stars.partial_cmp(&m2.stars).unwrap_or(Ordering::Equal),
                    })
            });

            (mapset, maps)
        }
        Err(OsuError::NotFound) => {
            let content = format!("Could find neither map nor mapset with id {mapset_id}");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let map_count = maps.len();

    let map_idx = if maps.is_empty() {
        return orig.error(&ctx, "The mapset has no maps").await;
    } else {
        map_id
            .and_then(|map_id| maps.iter().position(|map| map.map_id == map_id))
            .unwrap_or(0)
    };

    let map = &maps[map_idx];

    // Try creating the strain graph for the map
    let bg_fut = async {
        let bytes = ctx.client().get_mapset_cover(&mapset.covers.cover).await?;

        Ok::<_, Error>(image::load_from_memory(&bytes)?.thumbnail_exact(W, H))
    };

    let graph = match tokio::join!(strain_values(&ctx, map.map_id, mods), bg_fut) {
        (Ok(strain_values), Ok(img)) => match graph(strain_values, img) {
            Ok(graph) => Some(graph),
            Err(err) => {
                warn!("{:?}", Report::new(err));

                None
            }
        },
        (Err(err), _) => {
            let report = Report::new(err).wrap_err("failed to create oppai values");
            warn!("{report:?}");

            None
        }
        (_, Err(err)) => {
            let report = Report::new(err).wrap_err("failed to retrieve graph background");
            warn!("{report:?}");

            None
        }
    };

    // Accumulate all necessary data
    let data_fut = MapEmbed::new(
        map,
        &mapset,
        mods,
        graph.is_none(),
        &attrs,
        &ctx,
        (map_idx + 1, map_count),
    );

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    // Sending the embed
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    let with_thumbnail = if let Some(bytes) = graph {
        builder = builder.attachment("map_graph.png", bytes);

        false
    } else {
        true
    };

    if let Some(content) = attrs.content() {
        builder = builder.content(content);
    }

    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Add mapset and maps to database
    let (mapset_result, maps_result) = tokio::join!(
        ctx.psql().insert_beatmapset(&mapset),
        ctx.psql().insert_beatmaps(maps.iter()),
    );

    if let Err(err) = mapset_result {
        warn!("{:?}", Report::new(err));
    }

    if let Err(err) = maps_result {
        warn!("{:?}", Report::new(err));
    }

    // Skip pagination if too few entries
    if map_count == 1 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MapPagination::new(
        response,
        mapset,
        maps,
        mods,
        map_idx,
        with_thumbnail,
        attrs,
        Arc::clone(&ctx),
    );

    let owner = author_id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn strain_values(ctx: &Context, map_id: u32, mods: GameMods) -> BotResult<Vec<(f64, f64)>> {
    let map_path = prepare_beatmap_file(ctx, map_id).await?;
    let map = Beatmap::from_path(map_path).await.map_err(PpError::from)?;
    let strains = map.strains(mods.bits());
    let section_len = strains.section_length;

    let strains = strains
        .strains
        .into_iter()
        .scan(0.0, |time, strain| {
            *time += section_len;

            Some((*time, strain))
        })
        .collect();

    Ok(strains)
}

fn graph(strains: Vec<(f64, f64)>, background: DynamicImage) -> Result<Vec<u8>, GraphError> {
    const LEN: usize = W as usize * H as usize;
    const STEPS: usize = 128;

    let first_strain = strains.first().map_or(0.0, |(v, _)| *v);
    let last_strain = strains.last().map_or(0.0, |(v, _)| *v);

    let knots: Vec<_> = strains.iter().map(|(time, _)| *time).collect();
    let elements: Vec<_> = strains.into_iter().map(|(_, strain)| strain).collect();
    let dist = (last_strain - first_strain) / STEPS as f64;

    let curve = Linear::builder().elements(elements).knots(knots).build()?;

    let strains: Vec<_> = iter::successors(Some(first_strain), |n| Some(n + dist))
        .take(STEPS)
        .zip(curve.take(STEPS))
        .collect();

    let last_strain = strains.last().map_or(0.0, |(v, _)| *v);

    let (min_strain, max_strain) = strains
        .iter()
        .fold((f64::MAX, f64::MIN), |(min, max), (_, strain)| {
            (min.min(*strain), max.max(*strain))
        });

    if max_strain <= std::f64::EPSILON {
        return Err(GraphError::InvalidStrainPoints);
    }

    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(17_i32)
            .build_cartesian_2d(first_strain..last_strain, min_strain..max_strain)?;

        // Get grayscale value to determine color for x axis
        let (width, height) = background.dimensions();
        let y = height.saturating_sub(10);

        let sum: u32 = (0..width)
            .map(|x| {
                let Luma([value]) = background.get_pixel(x, y).to_luma();

                value as u32
            })
            .sum();

        let axis_color = if sum / width >= 128 { &BLACK } else { &WHITE };

        // Add background
        let background = background.blur(2.0).brighten(-20);
        let elem: BitMapElement<'_, _> = ((0.0_f64, max_strain), background).into();
        chart.draw_series(iter::once(elem))?;

        // Mesh and labels
        let text_style = FontDesc::new(FontFamily::Serif, 14.0, FontStyle::Bold).color(axis_color);

        chart
            .configure_mesh()
            .disable_y_mesh()
            .disable_y_axis()
            .set_all_tick_mark_size(3_i32)
            .light_line_style(axis_color.mix(0.0)) // hide
            .bold_line_style(axis_color.mix(0.4))
            .x_labels(10)
            .x_label_style(text_style)
            .x_label_formatter(&|timestamp| {
                if timestamp.abs() <= f64::EPSILON {
                    return String::new();
                }

                let d = Duration::from_millis(*timestamp as u64);
                let minutes = d.as_secs() / 60;
                let seconds = d.as_secs() % 60;

                format!("{minutes}:{seconds:0>2}")
            })
            .draw()?;

        // Draw line
        let glowing = GlowingPath::new(strains, Line);
        chart.draw_series(iter::once(glowing))?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, W, H, ColorType::Rgb8)?;

    Ok(png_bytes)
}

struct GlowingPath<Coord>(PathElement<Coord>);

impl<Coord> GlowingPath<Coord> {
    fn new(points: Vec<Coord>, style: impl Into<ShapeStyle>) -> Self {
        Self(PathElement::new(points, style))
    }
}

impl<'a, Coord> PointCollection<'a, Coord> for &'a GlowingPath<Coord> {
    type Point = &'a Coord;
    type IntoIter = &'a [Coord];

    fn point_iter(self) -> Self::IntoIter {
        self.0.point_iter()
    }
}

impl<DB, Coord> Drawable<DB> for GlowingPath<Coord>
where
    DB: DrawingBackend,
{
    fn draw<I>(
        &self,
        pos: I,
        backend: &mut DB,
        parent_dim: (u32, u32),
    ) -> Result<(), DrawingErrorKind<DB::ErrorType>>
    where
        I: Iterator<Item = BackendCoord>,
    {
        let pos: Vec<_> = pos.collect();
        backend.draw_path(pos.iter().copied(), &Glow)?;
        self.0.draw(pos.into_iter(), backend, parent_dim)?;

        Ok(())
    }
}

struct Glow;

impl BackendStyle for Glow {
    fn color(&self) -> BackendColor {
        BackendColor {
            alpha: 0.4,
            rgb: (255, 255, 255),
        }
    }

    fn stroke_width(&self) -> u32 {
        5
    }
}

struct Line;

impl From<Line> for ShapeStyle {
    fn from(_: Line) -> Self {
        Self {
            color: RGBColor(0, 255, 119).mix(1.0),
            filled: false,
            stroke_width: 2,
        }
    }
}

use std::{cmp::Ordering, iter, sync::Arc, time::Duration};

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
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    channel::message::MessageType,
};

use crate::{
    commands::{
        osu::{option_map, option_mods},
        MyCommand,
    },
    embeds::{EmbedData, MapEmbed},
    error::{GraphError, PpError},
    pagination::{MapPagination, Pagination},
    util::{
        constants::{
            common_literals::{MAP, MAP_PARSE_FAIL, MODS, MODS_PARSE_FAIL},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher,
        osu::{
            map_id_from_history, map_id_from_msg, prepare_beatmap_file, MapIdType, ModSelection,
        },
        ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

const W: u32 = 590;
const H: u32 = 150;

#[command]
#[short_desc("Display a bunch of stats about a map(set)")]
#[long_desc(
    "Display stats about a beatmap. Mods can be specified.\n\
    If no map(set) is specified by either url or id, I will choose the last map \
    I can find in the embeds of this channel.\n\
    If the mapset is specified by id but there is some map with the same id, \
    I will choose the latter."
)]
#[usage("[map(set) url / map(set) id] [+mods]")]
#[example("2240404 +hddt", "https://osu.ppy.sh/beatmapsets/902425 +hr")]
#[aliases("m", "beatmap", "maps", "beatmaps", "mapinfo")]
async fn map(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match MapArgs::args(&mut args) {
            Ok(mut map_args) => {
                let reply = msg
                    .referenced_message
                    .as_ref()
                    .filter(|_| msg.kind == MessageType::Reply);

                if let Some(id) = reply.and_then(|msg| map_id_from_msg(msg)) {
                    map_args.map = Some(id);
                }

                _map(ctx, CommandData::Message { msg, args, num }, map_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_map(ctx, *command).await,
    }
}

async fn _map(ctx: Arc<Context>, data: CommandData<'_>, args: MapArgs) -> BotResult<()> {
    let MapArgs { map, mods } = args;
    let author_id = data.author()?.id;

    let map_id = if let Some(id) = map {
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
                    Try specifying a map(set) either by url to the map, \
                    or just by map(set) id.";

                return data.error(&ctx, content).await;
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
                        Ok(map) => (map.mapset_id, Some(id)),
                        Err(OsuError::NotFound) => (id, None),
                        Err(why) => {
                            let _ = data.error(&ctx, OSU_API_ISSUE).await;

                            return Err(why.into());
                        }
                    }
                }
            }
        }

        // If its already given as mapset id, do nothing
        MapIdType::Set(id) => (id, None),
    };

    // Request mapset through API
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

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let map_count = maps.len();

    let map_idx = if maps.is_empty() {
        return data.error(&ctx, "The mapset has no maps").await;
    } else {
        map_id
            .and_then(|map_id| maps.iter().position(|map| map.map_id == map_id))
            .unwrap_or(0)
    };

    let map = &maps[map_idx];

    // Try creating the strain graph for the map
    let bg_fut = async {
        let bytes = ctx.clients.custom.get_mapset_cover(&mapset.covers).await?;

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
        (Err(why), _) => {
            let report = Report::new(why).wrap_err("failed to create oppai values");
            warn!("{report:?}");

            None
        }
        (_, Err(why)) => {
            let report = Report::new(why).wrap_err("failed to retrieve graph background");
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
        &ctx,
        (map_idx + 1, map_count),
    );

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sending the embed
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph.as_deref() {
        builder = builder.file("map_graph.png", bytes);
    }

    let response_raw = data.create_message(&ctx, builder).await?;

    // Add mapset and maps to database
    let (mapset_result, maps_result) = tokio::join!(
        ctx.clients.psql.insert_beatmapset(&mapset),
        ctx.clients.psql.insert_beatmaps(maps.iter()),
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
        graph.is_none(),
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
        let background = background.blur(2.0).brighten(-15);
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

struct MapArgs {
    map: Option<MapIdType>,
    mods: Option<ModSelection>,
}

impl MapArgs {
    fn args(args: &mut Args<'_>) -> Result<Self, String> {
        let mut map = None;
        let mut mods = None;

        for arg in args.take(2) {
            if let Some(id) =
                matcher::get_osu_map_id(arg).or_else(|| matcher::get_osu_mapset_id(arg))
            {
                map = Some(id);
            } else if let Some(mods_) = matcher::get_mods(arg) {
                mods = Some(mods_);
            } else {
                let content = format!(
                    "Failed to parse `{arg}`.\n\
                    Be sure you specify either a valid map id, map url, or mod combination."
                );

                return Err(content);
            }
        }

        Ok(Self { map, mods })
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, &'static str>> {
        let mut map = None;
        let mut mods = None;

        for option in command.yoink_options() {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MAP => match matcher::get_osu_map_id(&value)
                        .or_else(|| matcher::get_osu_mapset_id(&value))
                    {
                        Some(id) => map = Some(id),
                        None => return Ok(Err(MAP_PARSE_FAIL)),
                    },
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => match value.parse() {
                            Ok(mods_) => mods = Some(ModSelection::Exact(mods_)),
                            Err(_) => return Ok(Err(MODS_PARSE_FAIL)),
                        },
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        Ok(Ok(Self { map, mods }))
    }
}

pub async fn slash_map(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match MapArgs::slash(&mut command)? {
        Ok(args) => _map(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_map() -> MyCommand {
    let map = option_map();
    let mods = option_mods(false);

    let help = "Display a bunch of stats about a map(set).\n\
        The values in the map info will be adjusted to mods.\n\
        Since discord does not allow images to be adjusted when editing messages, \
        the strain graph always belongs to the initial map, even after moving to \
        other maps of the set through the arrow reactions.";

    MyCommand::new(MAP, "Display a bunch of stats about map(set)")
        .help(help)
        .options(vec![map, mods])
}

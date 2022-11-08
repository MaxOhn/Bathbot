use std::{borrow::Cow, cmp::Ordering, fmt::Write, iter, sync::Arc, time::Duration};

use command_macros::{command, HasMods, SlashCommand};
use enterpolation::{linear::Linear, Curve};
use eyre::{Report, Result, WrapErr};
use image::{
    codecs::png::PngEncoder, ColorType, DynamicImage, GenericImageView, ImageEncoder, Luma, Pixel,
};
use plotters::{
    element::{Drawable, PointCollection},
    prelude::*,
};
use plotters_backend::{BackendColor, BackendCoord, BackendStyle, DrawingErrorKind};
use rosu_pp::{Beatmap, BeatmapExt, Strains};
use rosu_v2::prelude::{GameMode, GameMods};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::channel::{message::MessageType, Message};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    pagination::MapPagination,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        matcher,
        osu::{prepare_beatmap_file, MapIdType},
        ChannelExt, InteractionCommandExt,
    },
    Context,
};

use super::{HasMods, ModsResult};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "map",
    help = "Display a bunch of stats about a map(set).\n\
    The values in the map info will be adjusted to mods.\n\
    Since discord does not allow images to be adjusted when editing messages, \
    the strain graph always belongs to the initial map, even after moving to \
    other maps of the set through the pagination buttons."
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
async fn prefix_map(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match MapArgs::args(msg, args) {
        Ok(args) => map(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_map(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Map::from_interaction(command.input_data())?;

    match MapArgs::try_from(args) {
        Ok(args) => map(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

const W: u32 = 590;
const H: u32 = 150;

async fn map(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: MapArgs<'_>) -> Result<()> {
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

    let map_id = if let Some(id) = map {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
            Ok(msgs) => msgs,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to retrieve channel history"));
            }
        };

        match MapIdType::from_msgs(&msgs, 0) {
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

    let mapset_res = match map_id {
        MapIdType::Map(id) => ctx.osu().beatmapset_from_map_id(id).await,
        MapIdType::Set(id) => ctx.osu().beatmapset(id).await,
    };

    let mut mapset = match mapset_res {
        Ok(mapset) => mapset,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get mapset"));
        }
    };

    if let Err(err) = ctx.osu_map().store(&mapset).await {
        warn!("{err:?}");
    }

    let Some(mut maps) = mapset.maps.take().filter(|maps| !maps.is_empty()) else {
        return orig.error(&ctx, "The mapset has no maps").await;
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

    // Try creating the strain graph for the map
    let bg_fut = async {
        let bytes = ctx.client().get_mapset_cover(&mapset.covers.cover).await?;

        let cover =
            image::load_from_memory(&bytes).wrap_err("failed to load mapset cover from memory")?;

        Ok::<_, Report>(cover.thumbnail_exact(W, H))
    };

    let graph = match tokio::join!(strain_values(&ctx, map_id, mods), bg_fut) {
        (Ok(strain_values), Ok(img)) => match graph(strain_values, img) {
            Ok(graph) => Some(graph),
            Err(err) => {
                warn!("{:?}", err.wrap_err("Failed to create graph"));

                None
            }
        },
        (Err(err), _) => {
            warn!("{:?}", err.wrap_err("Failed to calculate strain values"));

            None
        }
        (_, Err(err)) => {
            warn!("{:?}", err.wrap_err("Failed to get graph background"));

            None
        }
    };

    let content = attrs.content();

    let mut builder = MapPagination::builder(mapset, maps, mods, map_idx, attrs);

    if let Some(bytes) = graph {
        builder = builder.attachment("map_graph.png", bytes);
    }

    if let Some(content) = content {
        builder = builder.content(content);
    }

    builder
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

async fn strain_values(ctx: &Context, map_id: u32, mods: GameMods) -> Result<Vec<(f64, f64)>> {
    let map_path = prepare_beatmap_file(ctx, map_id)
        .await
        .wrap_err("failed to prepare map")?;

    let map = Beatmap::from_path(map_path)
        .await
        .wrap_err("failed to parse map")?;

    let strains = map.strains(mods.bits());
    let section_len = strains.section_len();

    let strains: Vec<(f64, f64)> = match strains {
        Strains::Catch(strains) => strains
            .movement
            .into_iter()
            .scan(0.0, |time, strain| {
                *time += section_len;

                Some((*time, strain))
            })
            .collect(),
        Strains::Mania(strains) => strains
            .strains
            .into_iter()
            .scan(0.0, |time, strain| {
                *time += section_len;

                Some((*time, strain))
            })
            .collect(),
        Strains::Osu(strains) => {
            let skill_count = (3 - mods.contains(GameMods::Relax) as usize
                + mods.contains(GameMods::Flashlight) as usize)
                as f64;

            strains
                .aim
                .into_iter()
                .zip(strains.aim_no_sliders)
                .zip(strains.speed)
                .zip(strains.flashlight)
                .map(|(((a, b), c), d)| (a + b + c + d) / skill_count)
                .scan(0.0, |time, strain| {
                    *time += section_len;

                    Some((*time, strain))
                })
                .collect()
        }
        Strains::Taiko(strains) => strains
            .color
            .into_iter()
            .zip(strains.rhythm)
            .zip(strains.stamina)
            .map(|((a, b), c)| (a + b + c) / 3.0)
            .scan(0.0, |time, strain| {
                *time += section_len;

                Some((*time, strain))
            })
            .collect(),
    };

    Ok(strains)
}

fn graph(strains: Vec<(f64, f64)>, background: DynamicImage) -> Result<Vec<u8>> {
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
        bail!("no non-zero strain point");
    }

    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE).wrap_err("failed to fill background")?;

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(17_i32)
            .build_cartesian_2d(first_strain..last_strain, min_strain..max_strain)
            .wrap_err("failed to build chart")?;

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
        chart
            .draw_series(iter::once(elem))
            .wrap_err("failed to draw background")?;

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
            .draw()
            .wrap_err("failed to draw mesh")?;

        // Draw line
        let glowing = GlowingPath::new(strains, Line);
        chart
            .draw_series(iter::once(glowing))
            .wrap_err("failed to draw path")?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);

    png_encoder
        .write_image(&buf, W, H, ColorType::Rgb8)
        .wrap_err("failed to encode image")?;

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

use std::{
    borrow::Cow, cell::RefCell, cmp::Ordering, fmt::Write, mem, rc::Rc, sync::Arc, time::Duration,
};

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::MapIdType,
    MessageOrigin,
};
use enterpolation::{linear::Linear, Curve};
use eyre::{ContextCompat, Report, Result, WrapErr};
use image::DynamicImage;
use plotters::{
    coord::{types::RangedCoordf64, Shift},
    prelude::*,
};
use plotters_skia::SkiaBackend;
use rosu_pp::{any::Strains, Difficulty};
use rosu_v2::prelude::{GameMode, GameModsIntermode, OsuError};
use skia_safe::{surfaces, BlendMode, EncodedImageFormat};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{message::MessageType, Message},
    guild::Permissions,
};

use super::{BitMapElement, HasMods, ModsResult};
use crate::{
    active::{impls::MapPagination, ActiveMessages},
    core::{
        commands::{prefix::Args, CommandOrigin},
        ContextExt,
    },
    util::{interaction::InteractionCommand, ChannelExt, CheckPermissions, InteractionCommandExt},
    Context,
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
    async fn args(ctx: &Context, msg: &Message, args: Args<'m>) -> Result<MapArgs<'m>, String> {
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

        if let Some(reply) = reply {
            if let Some(id) = ctx.find_map_id_in_msg(reply).await {
                map = Some(id);
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
async fn prefix_map(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match MapArgs::args(&ctx, msg, args).await {
        Ok(args) => map(ctx, CommandOrigin::from_msg(msg, permissions), args).await,
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
const H: u32 = 170;
const LEGEND_H: u32 = 25;

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
    } else if orig.can_read_history() {
        let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
            Ok(msgs) => msgs,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to retrieve channel history"));
            }
        };

        match ctx.find_map_id_in_msgs(&msgs, 0).await {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map(set) either by url to the map, \
                    or just by map(set) id.";

                return orig.error(&ctx, content).await;
            }
        }
    } else {
        let content =
            "No beatmap specified and lacking permission to search the channel history for maps.\n\
            Try specifying a map(set) either by url to the map, \
            or just by map(set) id, or give me the \"Read Message History\" permission.";

        return orig.error(&ctx, content).await;
    };

    let mods = match mods {
        Some(selection) => selection.into_mods(),
        None => GameModsIntermode::new(),
    };

    let mapset_res = match map_id {
        MapIdType::Map(id) => ctx.osu().beatmapset_from_map_id(id).await,
        MapIdType::Set(id) => ctx.osu().beatmapset(id).await,
    };

    let mut mapset = match mapset_res {
        Ok(mapset) => mapset,
        Err(OsuError::NotFound) => {
            let content = match map_id {
                MapIdType::Map(id) => format!("Beatmapset of map {id} was not found"),
                MapIdType::Set(id) => format!("Beatmapset with id {id} was not found"),
            };

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get mapset"));
        }
    };

    let mapset_clone = mapset.clone();
    let ctx_clone = ctx.cloned();
    tokio::spawn(async move { ctx_clone.osu_map().store(&mapset_clone).await });

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
    let mode = maps[map_idx].mode;

    if let Some(mods) = mods.clone().with_mode(mode) {
        if !mods.is_valid() {
            let content =
                format!("Looks like some mods in `{mods}` are incompatible with each other");

            return orig.error(&ctx, content).await;
        }
    } else {
        let content = format!(
            "The mods `{mods}` are incompatible with the map's mode {:?}",
            maps[map_idx].mode
        );

        return orig.error(&ctx, content).await;
    }

    // Try creating the strain graph for the map
    let bg_fut = async {
        let bytes = ctx.client().get_mapset_cover(&mapset.covers.cover).await?;

        let cover =
            image::load_from_memory(&bytes).wrap_err("failed to load mapset cover from memory")?;

        Ok::<_, Report>(cover.thumbnail_exact(W, H))
    };

    let (strain_values_res, img_res) =
        tokio::join!(strain_values(ctx.cloned(), map_id, &mods), bg_fut);

    let img_opt = match img_res {
        Ok(img) => Some(img),
        Err(err) => {
            warn!(?err, "Failed to get graph background");

            None
        }
    };

    let graph = match strain_values_res {
        Ok(strain_values) => match graph(strain_values, img_opt) {
            Ok(graph) => Some(graph),
            Err(err) => {
                warn!(?err, "Failed to create graph");

                None
            }
        },
        Err(err) => {
            warn!(?err, "Failed to calculate strain values");

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
        .begin(ctx, orig)
        .await
}

struct GraphStrains {
    /// Smoothed strain values
    strains: Strains,
    /// The initial amount of strains
    strains_count: usize,
}

const NEW_STRAIN_COUNT: usize = 200;

async fn strain_values(
    ctx: Arc<Context>,
    map_id: u32,
    mods: &GameModsIntermode,
) -> Result<GraphStrains> {
    let map = ctx
        .osu_map()
        .pp_map(map_id)
        .await
        .wrap_err("failed to get pp map")?;

    let mut strains = Difficulty::new().mods(mods.bits()).strains(&map);
    let section_len = strains.section_len();

    let strains_count = match strains {
        Strains::Osu(ref strains) => strains.aim.len(),
        Strains::Taiko(ref strains) => strains.color.len(),
        Strains::Catch(ref strains) => strains.movement.len(),
        Strains::Mania(ref strains) => strains.strains.len(),
    };

    let create_curve = |strains: Vec<f64>| {
        Linear::builder()
            .elements(strains)
            .equidistant()
            .distance(0.0, section_len)
            .build()
            .map(|curve| curve.take(NEW_STRAIN_COUNT).collect())
    };

    match &mut strains {
        Strains::Osu(strains) => {
            strains
                .aim
                .iter()
                .zip(strains.aim_no_sliders.iter_mut())
                .for_each(|(aim, no_slider)| *no_slider = *aim - *no_slider);

            strains.aim =
                create_curve(mem::take(&mut strains.aim)).wrap_err("Failed to build aim curve")?;
            strains.aim_no_sliders = create_curve(mem::take(&mut strains.aim_no_sliders))
                .wrap_err("Failed to build aim_no_sliders curve")?;
            strains.speed = create_curve(mem::take(&mut strains.speed))
                .wrap_err("Failed to build speed curve")?;
            strains.flashlight = create_curve(mem::take(&mut strains.flashlight))
                .wrap_err("Failed to build flashlight curve")?;
        }
        Strains::Taiko(strains) => {
            strains.color = create_curve(mem::take(&mut strains.color))
                .wrap_err("Failed to build color curve")?;
            strains.rhythm = create_curve(mem::take(&mut strains.rhythm))
                .wrap_err("Failed to build rhythm curve")?;
            strains.stamina = create_curve(mem::take(&mut strains.stamina))
                .wrap_err("Failed to build stamina curve")?;
        }
        Strains::Catch(strains) => {
            strains.movement = create_curve(mem::take(&mut strains.movement))
                .wrap_err("Failed to build movement curve")?;
        }
        Strains::Mania(strains) => {
            strains.strains = create_curve(mem::take(&mut strains.strains))
                .wrap_err("Failed to build strains curve")?;
        }
    }

    Ok(GraphStrains {
        strains,
        strains_count,
    })
}

fn graph(strains: GraphStrains, background: Option<DynamicImage>) -> Result<Vec<u8>> {
    let last_timestamp = ((NEW_STRAIN_COUNT - 2) as f64
        * strains.strains.section_len()
        * strains.strains_count as f64)
        / NEW_STRAIN_COUNT as f64;

    let max_strain = match &strains.strains {
        Strains::Osu(strains) => strains
            .aim
            .iter()
            .zip(strains.aim_no_sliders.iter())
            .zip(strains.speed.iter())
            .zip(strains.flashlight.iter())
            .fold(0.0_f64, |max, (((a, b), c), d)| {
                max.max(*a).max(*b).max(*c).max(*d)
            }),
        Strains::Taiko(strains) => strains
            .color
            .iter()
            .zip(strains.rhythm.iter())
            .zip(strains.stamina.iter())
            .fold(0.0_f64, |max, ((a, b), c)| max.max(*a).max(*b).max(*c)),
        Strains::Catch(strains) => strains
            .movement
            .iter()
            .fold(0.0_f64, |max, strain| max.max(*strain)),
        Strains::Mania(strains) => strains
            .strains
            .iter()
            .fold(0.0_f64, |max, strain| max.max(*strain)),
    };

    if max_strain <= std::f64::EPSILON {
        bail!("no non-zero strain point");
    }

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let backend = Rc::new(RefCell::new(SkiaBackend::new(surface.canvas(), W, H)));
        let root = DrawingArea::from(&backend);

        // Add background
        if let Some(background) = background {
            let background = background.blur(2.0);
            let elem = BitMapElement::new(background, (0, 0));
            root.draw(&elem).wrap_err("Failed to draw background")?;

            let rect = Rectangle::new([(0, 0), (W as i32, H as i32)], BLACK.mix(0.75).filled());
            root.draw(&rect)
                .wrap_err("Failed to draw darkening rectangle")?;
        } else {
            root.fill(&RGBColor(19, 43, 33))
                .wrap_err("Failed to fill background")?;
        }

        let (legend_area, graph_area) = root.split_vertically(LEGEND_H);

        let mut chart = ChartBuilder::on(&graph_area)
            .x_label_area_size(17_i32)
            .build_cartesian_2d(last_timestamp.min(1.0)..last_timestamp, 0.0_f64..max_strain)
            .wrap_err("Failed to build chart")?;

        // Mesh and labels
        let text_style = FontDesc::new(FontFamily::SansSerif, 14.0, FontStyle::Bold).color(&WHITE);

        chart
            .configure_mesh()
            .disable_y_mesh()
            .disable_y_axis()
            .set_all_tick_mark_size(3_i32)
            .light_line_style(WHITE.mix(0.0)) // hide
            .bold_line_style(WHITE.mix(0.75))
            .x_labels(10)
            .x_label_style(text_style.clone())
            .axis_style(WHITE)
            .x_label_formatter(&|timestamp| {
                if timestamp.abs() < f64::EPSILON {
                    return String::new();
                }

                let d = Duration::from_millis(*timestamp as u64);
                let minutes = d.as_secs() / 60;
                let seconds = d.as_secs() % 60;

                format!("{minutes}:{seconds:0>2}")
            })
            .draw()
            .wrap_err("Failed to draw mesh")?;

        draw_mode_strains(&backend, &mut chart, strains, &legend_area, &text_style)?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

fn draw_mode_strains(
    backend: &Rc<RefCell<SkiaBackend<'_>>>,
    chart: &mut ChartContext<'_, SkiaBackend<'_>, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    strains: GraphStrains,
    legend_area: &DrawingArea<SkiaBackend<'_>, Shift>,
    text_style: &TextStyle<'_>,
) -> Result<()> {
    let GraphStrains {
        strains,
        strains_count,
    } = strains;

    let orig_count = strains_count as f64;

    let new_count = match strains {
        Strains::Osu(ref strains) => strains.aim.len(),
        Strains::Taiko(ref strains) => strains.color.len(),
        Strains::Catch(ref strains) => strains.movement.len(),
        Strains::Mania(ref strains) => strains.strains.len(),
    } as f64;

    let section_len = strains.section_len();

    let mut legend_x: i32 = 8;

    let factor = section_len * orig_count / new_count;

    macro_rules! draw_line {
        ( $label:literal, $strains:expr, $color:ident ) => {{
            draw_series(backend, chart, &$strains, $label, factor, $color)?;
            draw_line(legend_area, $label, $color, text_style, &mut legend_x)?;
        }};
    }

    fn draw_series(
        backend: &Rc<RefCell<SkiaBackend<'_>>>,
        chart: &mut ChartContext<'_, SkiaBackend<'_>, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        strains: &[f64],
        label: &str,
        factor: f64,
        color: RGBColor,
    ) -> Result<()> {
        backend
            .borrow_mut()
            .set_blend_mode(Some(BlendMode::Lighten));

        let timestamp_iter = strains
            .iter()
            .enumerate()
            .map(move |(i, strain)| (i as f64 * factor, *strain));

        let series = AreaSeries::new(timestamp_iter, 0.0, color.mix(0.20))
            .border_style(color.stroke_width(2));

        chart
            .draw_series(series)
            .wrap_err_with(|| format!("Failed to draw {label} series"))?;

        backend.borrow_mut().set_blend_mode(None);

        Ok(())
    }

    fn draw_line(
        legend_area: &DrawingArea<SkiaBackend<'_>, Shift>,
        label: &str,
        color: RGBColor,
        text_style: &TextStyle<'_>,
        legend_x: &mut i32,
    ) -> Result<()> {
        let rect = Rectangle::new(
            [
                (*legend_x, (LEGEND_H as f32 * 0.42) as i32),
                (*legend_x + 16, (LEGEND_H as f32 * 0.58) as i32),
            ],
            color.filled(),
        );

        legend_area
            .draw(&rect)
            .wrap_err("Failed to draw legend rectangle")?;

        *legend_x += 26;

        let ((min_x, min_y), (max_x, max_y)) = text_style
            .font
            .layout_box(label)
            .wrap_err("Failed to get legend layout box")?;

        let width = max_x - min_x;
        let height = max_y - min_y;

        let text_pos = (*legend_x, (LEGEND_H as i32 - 8 - height));

        legend_area
            .draw_text(label, text_style, text_pos)
            .wrap_err("Failed to draw legend text")?;

        *legend_x += width + 10;

        Ok(())
    }

    match strains {
        Strains::Osu(strains) => {
            draw_line!("Aim", strains.aim, CYAN);
            draw_line!("Aim (Sliders)", strains.aim_no_sliders, GREEN);
            draw_line!("Speed", strains.speed, RED);
            draw_line!("Flashlight", strains.flashlight, MAGENTA);
        }
        Strains::Taiko(strains) => {
            draw_line!("Stamina", strains.stamina, RED);
            draw_line!("Color", strains.color, YELLOW);
            draw_line!("Rhythm", strains.rhythm, CYAN);
        }
        Strains::Catch(strains) => draw_line!("Movement", strains.movement, CYAN),
        Strains::Mania(strains) => draw_line!("Strain", strains.strains, MAGENTA),
    }

    Ok(())
}

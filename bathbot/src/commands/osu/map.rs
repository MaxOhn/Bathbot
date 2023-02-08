use std::{borrow::Cow, cmp::Ordering, fmt::Write, mem, sync::Arc, time::Duration};

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::MapIdType,
};
use enterpolation::{linear::Linear, Curve};
use eyre::{Report, Result, WrapErr, ContextCompat};
use image::DynamicImage;
use rosu_pp::{BeatmapExt, Strains};
use rosu_v2::prelude::{GameMode, GameMods, OsuError};
use skia_safe::{Paint, Surface, Path, PaintStyle, EncodedImageFormat, Rect, Image, Data, ImageInfo, Color, TileMode, BlendMode, Font, Typeface, FontStyle, TextBlob, PaintCap, gradient_shader};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{message::MessageType, Message},
    guild::Permissions,
};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::MessageOrigin,
    pagination::MapPagination,
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
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
async fn prefix_map(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match MapArgs::args(msg, args) {
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
const GRAPH_H: u32 = H - LEGEND_H - 21;

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

        match MapIdType::from_msgs(&msgs, 0) {
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
        Some(selection) => selection.mods(),
        None => GameMods::NoMod,
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

    let (strain_values_res, img_res) = tokio::join!(strain_values(&ctx, map_id, mods), bg_fut);

    let img_opt = match img_res {
        Ok(img) => Some(img),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to get graph background"));

            None
        }
    };

    let graph = match strain_values_res {
        Ok(strain_values) => match graph(strain_values, img_opt) {
            Ok(graph) => Some(graph),
            Err(err) => {
                warn!("{:?}", err.wrap_err("Failed to create graph"));

                None
            }
        },
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to calculate strain values"));

            None
        }
    };

    let content = attrs.content();

    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());
    let mut builder = MapPagination::builder(mapset, maps, mods, map_idx, attrs, origin);

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

struct GraphStrains {
    strains: Strains,
    strains_count: usize,
}

const NEW_STRAIN_COUNT: usize = 200;

async fn strain_values(ctx: &Context, map_id: u32, mods: GameMods) -> Result<GraphStrains> {
    let map = ctx
        .osu_map()
        .pp_map(map_id)
        .await
        .wrap_err("failed to get pp map")?;

    let mut strains = map.strains(mods.bits());
    let section_len = strains.section_len();
    let strains_count = strains.len();

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

    let mut surface = Surface::new_raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;
    let mut paint = Paint::default();
    paint
        .set_color(Color::BLACK)
        .set_style(PaintStyle::Fill)
        .set_anti_alias(true);

    surface.canvas().draw_rect(Rect::new(0., 0., W as f32, H as f32), &paint);

    if let Some(background) = background {
        let info = ImageInfo::new(
            (background.width() as i32, background.height() as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Opaque,
            None
        );

        let bytes = background.blur(3.0).to_rgba8();
        let pixels = Data::new_copy(&bytes);
        let row_bytes = background.width() * 4;

        let image = Image::from_raster_data(&info, pixels, row_bytes as usize).wrap_err("Failed to create image")?;
        surface.canvas().draw_image_rect(image, None, Rect::new(0., 0., W as f32, H as f32), &paint);
    }

    paint.set_alpha_f(0.7);
    surface.canvas().draw_rect(Rect::new(0., 0., W as f32, H as f32), &paint);

    // Maybe change the font?
    let typeface = Typeface::new("Comfortaa", FontStyle::bold()).wrap_err("Failed to get typeface")?;
    let font = Font::from_typeface(typeface, Some(13.0));

    draw_mode_strains(&mut surface, strains, max_strain, font)?;

    paint
        .set_color(Color::WHITE)
        .set_alpha_f(1.0)
        .set_style(PaintStyle::Stroke)
        .set_stroke_width(1.0);

    surface.canvas().draw_line((0., (LEGEND_H + GRAPH_H) as f32), (W as f32, (LEGEND_H + GRAPH_H) as f32), &paint);

    let data = surface.image_snapshot().encode_to_data(EncodedImageFormat::PNG).wrap_err("Failed to encode image")?;
    let png_bytes = data.as_bytes().to_vec();

    Ok(png_bytes)
}

fn draw_strain(
    surface: &mut Surface,
    label: &str,
    strains: Vec<f64>,
    max_strain: f64,
    color: Color,
    font: &Font,
    legend_x: &mut f32
) -> Result<()> {
    let gradient = gradient_shader::linear(
        ((0., LEGEND_H as f32), (0., (LEGEND_H + GRAPH_H) as f32)),
        [color.with_a(125), color.with_a(20)].as_slice(),
        None,
        TileMode::Clamp,
        None,
        None
    ).wrap_err("Failed to create gradient shader")?;

    let mut paint = Paint::default();
    paint
        .set_color(color)
        .set_alpha_f(0.75)
        .set_anti_alias(true)
        .set_stroke_width(2.0)
        .set_stroke_cap(skia_safe::PaintCap::Round)
        .set_stroke_join(skia_safe::PaintJoin::Round)
        .set_style(PaintStyle::Stroke)
        .set_blend_mode(BlendMode::Lighten);

    let mut path = Path::new();
    path.move_to((0., (LEGEND_H + GRAPH_H) as f32));

    let len = strains.len();
    for (i, strain) in strains.iter().enumerate() {
        path.line_to((
            (i as f32 / (len - 1) as f32) * W as f32,
            LEGEND_H as f32 + GRAPH_H as f32 - *strain as f32 / max_strain as f32 * GRAPH_H as f32
        ));
    }

    surface.canvas().draw_path(&path, &paint);

    path.line_to((W as f32, (LEGEND_H + GRAPH_H) as f32));

    paint
        .set_shader(Some(gradient))
        .set_style(PaintStyle::Fill);

    surface.canvas().draw_path(&path, &paint);

    paint
        .set_shader(None)
        .set_blend_mode(BlendMode::default())
        .set_color(color.with_a(170));

    surface.canvas().draw_rect(Rect::new(*legend_x, LEGEND_H as f32 * 0.42, *legend_x + 16.0, LEGEND_H as f32 * 0.58), &paint);

    *legend_x += 26.;

    paint.set_color(Color::WHITE);

    let textblob = TextBlob::from_str(label, font).wrap_err("Failed to create text blob")?;
    surface.canvas().draw_text_blob(&textblob, (*legend_x, (LEGEND_H) as f32 - 8.0), &paint);

    paint.set_alpha(50);

    let (_, bounds) = font.measure_str(label, Some(&paint));

    *legend_x += bounds.width() + 10.0;

    Ok(())
}

fn draw_mode_strains(
    surface: &mut Surface,
    strains: GraphStrains,
    max_strain: f64,
    font: Font
) -> Result<()> {
    let GraphStrains {
        strains,
        strains_count,
    } = strains;

    let length = strains_count as f64 * strains.section_len();

    let mut paint = Paint::default();
    paint.set_color(Color::WHITE)
        .set_stroke_width(2.0)
        .set_stroke_cap(PaintCap::Round);

    let format_timestamp = |timestamp: f32| {
        if timestamp.abs() <= f32::EPSILON {
            return String::new()
        }

        let d = Duration::from_millis(timestamp as u64);
        let hours = d.as_secs() / 3600;
        let minutes = d.as_secs() / 60 % 60;
        let seconds = d.as_secs() % 60;

        if hours > 0 {
            format!("{hours}:{minutes:0>2}:{seconds:0>2}")
        } else {
            format!("{minutes}:{seconds:0>2}")
        }
    };

    for i in 1..=6 {
        let k = i as f32 / 6.;

        paint.set_alpha_f(0.4);
        surface.canvas().draw_line((W as f32 * k, (LEGEND_H + GRAPH_H) as f32), (W as f32 * k, LEGEND_H as f32 - 0.5), &paint);
        let timestamp = length as f32 * k;
        
        let label = format_timestamp(timestamp);
        let textblob = TextBlob::from_str(label.as_str(), &font).wrap_err("Failed to create text blob")?;

        let (_, bounds) = font.measure_str(label.as_str(), Some(&paint));
        let offset = match i { 6 => bounds.width(), _ => bounds.width() / 2.0 };
        
        paint.set_alpha_f(1.0);
        surface.canvas().draw_text_blob(&textblob, (W as f32 * k - offset, H as f32 - 4.0), &paint);
    }

    let mut legend_x: f32 = 8.0;

    match strains {
        Strains::Osu(strains) => {
            draw_strain(surface, "Aim", strains.aim, max_strain, Color::CYAN, &font, &mut legend_x)?;
            draw_strain(surface, "Aim (Sliders)", strains.aim_no_sliders, max_strain, Color::GREEN, &font, &mut legend_x)?;
            draw_strain(surface, "Speed", strains.speed, max_strain, Color::RED, &font, &mut legend_x)?;
            draw_strain(surface, "Flashlight", strains.flashlight, max_strain, Color::MAGENTA, &font, &mut legend_x)?;
        }
        Strains::Taiko(strains) => {
            draw_strain(surface, "Stamina", strains.stamina, max_strain, Color::RED, &font, &mut legend_x)?;
            draw_strain(surface, "Color", strains.color, max_strain, Color::YELLOW, &font, &mut legend_x)?;
            draw_strain(surface, "Rhythm", strains.rhythm, max_strain, Color::CYAN, &font, &mut legend_x)?;
        }
        Strains::Catch(strains) => draw_strain(surface, "Movement", strains.movement, max_strain, Color::CYAN, &font, &mut legend_x)?,
        Strains::Mania(strains) => draw_strain(surface, "Strain", strains.strains, max_strain, Color::MAGENTA, &font, &mut legend_x)?,
    };

    Ok(())
}

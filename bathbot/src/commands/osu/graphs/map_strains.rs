use std::{cell::RefCell, mem, rc::Rc, time::Duration};

use enterpolation::{Curve, linear::Linear};
use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    coord::{Shift, types::RangedCoordf64},
    prelude::*,
};
use plotters_skia::SkiaBackend;
use rosu_pp::{Beatmap, Difficulty, any::Strains};
use rosu_v2::prelude::GameMods;
use skia_safe::{BlendMode, EncodedImageFormat, surfaces};

use super::{BitMapElement, get_map_cover};

const LEGEND_H: u32 = 25;

pub async fn map_strains_graph(
    map: &Beatmap,
    mods: GameMods,
    cover_url: &str,
    w: u32,
    h: u32,
) -> Result<Vec<u8>> {
    let strains = GraphStrains::new(map, mods)?;
    let cover_res = get_map_cover(cover_url, w, h).await;

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
            .zip(strains.single_color_stamina.iter())
            .fold(0.0_f64, |max, (((a, b), c), d)| {
                max.max(*a).max(*b).max(*c).max(*d)
            }),
        Strains::Catch(strains) => strains
            .movement
            .iter()
            .fold(0.0_f64, |max, strain| max.max(*strain)),
        Strains::Mania(strains) => strains
            .strains
            .iter()
            .fold(0.0_f64, |max, strain| max.max(*strain)),
    };

    if max_strain <= f64::EPSILON {
        bail!("no non-zero strain point");
    }

    let mut surface =
        surfaces::raster_n32_premul((w as i32, h as i32)).wrap_err("Failed to create surface")?;

    {
        let backend = Rc::new(RefCell::new(SkiaBackend::new(surface.canvas(), w, h)));
        let root = DrawingArea::from(&backend);

        // Add background
        match cover_res {
            Ok(background) => {
                let background = background.blur(2.0);
                let elem = BitMapElement::new(background, (0, 0));
                root.draw(&elem).wrap_err("Failed to draw background")?;

                let rect = Rectangle::new([(0, 0), (w as i32, h as i32)], BLACK.mix(0.75).filled());
                root.draw(&rect)
                    .wrap_err("Failed to draw darkening rectangle")?;
            }
            Err(err) => {
                warn!(?err, "Failed to get mapset cover");

                root.fill(&RGBColor(19, 43, 33))
                    .wrap_err("Failed to fill background")?;
            }
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
            draw_line!("Stamina (Single color)", strains.single_color_stamina, BLUE);
            draw_line!("Color", strains.color, YELLOW);
            draw_line!("Rhythm", strains.rhythm, CYAN);
            draw_line!("Reading", strains.reading, GREEN);
        }
        Strains::Catch(strains) => draw_line!("Movement", strains.movement, CYAN),
        Strains::Mania(strains) => draw_line!("Strain", strains.strains, MAGENTA),
    }

    Ok(())
}

const NEW_STRAIN_COUNT: usize = 200;

struct GraphStrains {
    /// Smoothed strain values
    strains: Strains,
    /// The initial amount of strains
    strains_count: usize,
}

impl GraphStrains {
    fn new(map: &Beatmap, mods: GameMods) -> Result<Self> {
        if map.check_suspicion().is_err() {
            bail!("skip strain calculation because map is too suspicious");
        }

        let mut strains = Difficulty::new().mods(mods).strains(map);
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

                strains.aim = create_curve(mem::take(&mut strains.aim))
                    .wrap_err("Failed to build aim curve")?;
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
                strains.single_color_stamina =
                    create_curve(mem::take(&mut strains.single_color_stamina))
                        .wrap_err("Failed to build single color stamina curve")?;
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

        Ok(Self {
            strains,
            strains_count,
        })
    }
}

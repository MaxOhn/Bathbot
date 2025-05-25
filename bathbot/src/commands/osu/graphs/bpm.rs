use std::{cell::RefCell, rc::Rc, time::Duration};

use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    chart::ChartBuilder,
    prelude::{DrawingArea, Rectangle},
    series::LineSeries,
    style::{BLACK, Color, FontDesc, RGBColor, WHITE},
};
use plotters_backend::{FontFamily, FontStyle};
use plotters_skia::SkiaBackend;
use rosu_pp::{
    Beatmap,
    model::{
        control_point::TimingPoint,
        hit_object::{HitObjectKind, HoldNote, Spinner},
    },
};
use rosu_v2::prelude::GameMods;
use skia_safe::{EncodedImageFormat, surfaces};

use super::{BitMapElement, H, W, get_map_cover};

pub async fn map_bpm_graph(map: &Beatmap, mods: GameMods, cover_url: &str) -> Result<Vec<u8>> {
    let mut start_timestamp = map
        .hit_objects
        .first()
        .zip(map.timing_points.first())
        .map(|(h, tp)| h.start_time.min(tp.time))
        .unwrap_or(0.0);

    let mut last_timestamp = map.hit_objects.last().map_or(0.0, |h| match h.kind {
        HitObjectKind::Circle | HitObjectKind::Slider(_) => h.start_time,
        HitObjectKind::Spinner(Spinner { duration })
        | HitObjectKind::Hold(HoldNote { duration }) => h.start_time + duration,
    });

    let start_bpm = map
        .timing_points
        .first()
        .map_or(TimingPoint::DEFAULT_BPM, TimingPoint::bpm);

    let mut points = Vec::with_capacity(2 * map.timing_points.len());

    if let Some(h) = map.hit_objects.first() {
        if map
            .timing_points
            .first()
            .is_some_and(|tp| tp.time > h.start_time)
        {
            points.push((h.start_time, start_bpm));
        }
    }

    let iter = map
        .timing_points
        .iter()
        .scan(start_bpm, |prev_bpm, tp| {
            let bpm = tp.bpm();
            let points = [(tp.time, *prev_bpm), (tp.time, bpm)];
            *prev_bpm = bpm;

            Some(points)
        })
        .flatten();

    points.extend(iter);

    if map
        .timing_points
        .last()
        .is_some_and(|tp| tp.time < last_timestamp)
    {
        let (_, last_bpm) = points[points.len() - 1];
        points.push((last_timestamp, last_bpm));
    } else if map.timing_points.is_empty() {
        points.extend([
            (0.0, TimingPoint::DEFAULT_BPM),
            (last_timestamp, TimingPoint::DEFAULT_BPM),
        ]);
    }

    let clock_rate = mods.clock_rate().unwrap_or(1.0);

    if clock_rate != 1.0 {
        for (time, bpm) in points.iter_mut() {
            *time /= clock_rate;
            *bpm *= clock_rate;
        }

        start_timestamp /= clock_rate;
        last_timestamp /= clock_rate;
    }

    let (min_bpm, max_bpm) = points
        .iter()
        .map(|(_, bpm)| (*bpm, *bpm))
        .reduce(|(min, max), (a, b)| (min.min(a), max.max(b)))
        .unwrap_or((TimingPoint::DEFAULT_BPM, TimingPoint::DEFAULT_BPM));

    let bpm_range = (max_bpm - min_bpm).max(5.0);
    let lower_limit = min_bpm - bpm_range * 0.2;
    let upper_limit = max_bpm + bpm_range * 0.2;

    let cover_res = get_map_cover(cover_url, W, H).await;

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let backend = Rc::new(RefCell::new(SkiaBackend::new(surface.canvas(), W, H)));
        let root = DrawingArea::from(&backend);

        // Add background
        match cover_res {
            Ok(background) => {
                let background = background.blur(2.0);
                let elem = BitMapElement::new(background, (0, 0));
                root.draw(&elem).wrap_err("Failed to draw background")?;

                let rect = Rectangle::new([(0, 0), (W as i32, H as i32)], BLACK.mix(0.75).filled());
                root.draw(&rect)
                    .wrap_err("Failed to draw darkening rectangle")?;
            }
            Err(err) => {
                warn!(?err, "Failed to get mapset cover");

                root.fill(&RGBColor(19, 43, 33))
                    .wrap_err("Failed to fill background")?;
            }
        }

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(22_i32)
            .y_label_area_size(70_i32)
            .margin_left(6)
            .build_cartesian_2d(start_timestamp..last_timestamp, lower_limit..upper_limit)
            .wrap_err("Failed to build chart")?;

        let text_style = FontDesc::new(FontFamily::SansSerif, 18.0, FontStyle::Bold).color(&WHITE);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .set_all_tick_mark_size(3_i32)
            .light_line_style(WHITE.mix(0.0)) // hide
            .bold_line_style(WHITE.mix(0.75))
            .x_labels(10)
            .x_label_style(text_style.clone())
            .y_label_style(text_style.clone())
            .axis_style(WHITE)
            .x_label_formatter(&|timestamp| {
                if timestamp.abs() < 0.0 {
                    return String::new();
                }

                let d = Duration::from_millis(*timestamp as u64);
                let minutes = d.as_secs() / 60;
                let seconds = d.as_secs() % 60;

                format!("{minutes}:{seconds:0>2}")
            })
            .y_desc("BPM")
            .y_label_formatter(&|bpm| ((bpm * 10.0).round() / 10.0).to_string())
            .draw()
            .wrap_err("Failed to draw mesh")?;

        let series = LineSeries::new(points.iter().copied(), WHITE.mix(0.3).stroke_width(6));
        chart
            .draw_series(series)
            .wrap_err("Failed to draw white series")?;

        let series = LineSeries::new(
            points.iter().copied(),
            RGBColor(0, 208, 138).stroke_width(2),
        );
        chart
            .draw_series(series)
            .wrap_err("Failed to draw green series")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

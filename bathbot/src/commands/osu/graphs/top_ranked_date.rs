use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    prelude::{ChartBuilder, Circle, EmptyElement, IntoDrawingArea, SeriesLabelPosition},
    series::PointSeries,
    style::{Color, RGBColor, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rosu_v2::prelude::Score;
use skia_safe::{EncodedImageFormat, surfaces};
use time::OffsetDateTime;

use super::{H, W};
use crate::{commands::osu::graphs::LegendDraw, util::Monthly};

/// Returns the ranked date of a score's mapset, if the score has one
pub fn ranked_date(score: &Score) -> Option<OffsetDateTime> {
    // `last_updated` is the only available datetime which should be what we
    // need considering top plays will all be ranked so that their
    // "last updated" timestamp should match the ranked date.
    score.map.as_ref().map(|map| map.last_updated)
}

pub async fn top_graph_ranked_date(caption: String, scores: &[Score]) -> Result<Vec<u8>> {
    let mut scored: Vec<(OffsetDateTime, &Score)> = Vec::new();

    for score in scores {
        if let Some(date) = ranked_date(score) {
            scored.push((date, score));
        }
    }

    scored.sort_unstable_by_key(|(date, _)| *date);

    if scored.is_empty() {
        bail!("user has no ranked maps in their top scores");
    }

    let max = scored
        .iter()
        .filter_map(|(_, score)| score.pp)
        .max_by(f32::total_cmp)
        .unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scored
        .iter()
        .filter_map(|(_, score)| score.pp)
        .min_by(f32::total_cmp)
        .unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    let first = scored[0].0;
    let last = scored[scored.len() - 1].0;

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let caption_style = ("sans-serif", 25_i32, FontStyle::Bold, &WHITE);

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(40_i32)
            .y_label_area_size(60_i32)
            .margin_top(5_i32)
            .margin_right(15_i32)
            .caption(format!("{caption} by ranked date"), caption_style)
            .build_cartesian_2d(Monthly(first..last), min_adj..max_adj)
            .wrap_err("failed to build chart")?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .y_label_formatter(&|pp| format!("{pp:.0}pp"))
            .x_label_formatter(&|datetime| datetime.date().to_string())
            .label_style(("sans-serif", 16_i32, &WHITE))
            .bold_line_style(WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw mesh")?;

        let point_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = WHITE.mix(0.9).stroke_width(1);

        let iter = scored
            .iter()
            .filter_map(|(date, score)| Some((*date, score.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, point_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw main points")?
            .label(format!("Max: {max}pp"))
            .legend(EmptyElement::at);

        let iter = scored
            .iter()
            .filter_map(|(date, score)| Some((*date, score.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, border_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw point borders")?
            .label(format!("Min: {min}pp"))
            .legend(EmptyElement::at);

        LegendDraw::new(&mut chart)
            .position(SeriesLabelPosition::MiddleLeft)
            .draw()?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

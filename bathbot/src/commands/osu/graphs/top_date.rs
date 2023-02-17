use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    prelude::{ChartBuilder, Circle, EmptyElement, IntoDrawingArea, SeriesLabelPosition},
    series::PointSeries,
    style::{Color, RGBColor, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rosu_v2::prelude::Score;
use skia_safe::{EncodedImageFormat, Surface};

use crate::util::Monthly;

use super::{H, W};

pub async fn top_graph_date(caption: String, scores: &mut [Score]) -> Result<Vec<u8>> {
    let max = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scores.last().and_then(|s| s.pp).unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    scores.sort_unstable_by_key(|s| s.ended_at);
    let dates: Vec<_> = scores.iter().map(|s| s.ended_at).collect();

    let first = dates[0];
    let last = dates[dates.len() - 1];

    let mut surface = Surface::new_raster_n32_premul((W as i32, H as i32))
        .wrap_err("Failed to create surface")?;

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
            .caption(caption, caption_style)
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

        let iter = scores.iter().filter_map(|s| Some((s.ended_at, s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, point_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw main points")?
            .label(format!("Max: {max}pp"))
            .legend(EmptyElement::at);

        let iter = scores.iter().filter_map(|s| Some((s.ended_at, s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, border_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw point borders")?
            .label(format!("Min: {min}pp"))
            .legend(EmptyElement::at);

        chart
            .configure_series_labels()
            .border_style(WHITE.mix(0.6).stroke_width(1))
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::MiddleLeft)
            .legend_area_size(0_i32)
            .label_font(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw legend")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode_to_data(EncodedImageFormat::PNG)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

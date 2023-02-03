use eyre::{Result, WrapErr};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::{
    prelude::{BitMapBackend, ChartBuilder, EmptyElement, IntoDrawingArea, SeriesLabelPosition},
    series::AreaSeries,
    style::{Color, RGBColor, WHITE},
};
use plotters_backend::FontStyle;
use rosu_v2::prelude::Score;

use super::{H, W};

pub async fn top_graph_index(caption: String, scores: &[Score]) -> Result<Vec<u8>> {
    let max = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scores.last().and_then(|s| s.pp).unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    let len = (W * H) as usize;
    let mut buf = vec![0; len * 3];

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
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
            .build_cartesian_2d(1..scores.len(), min_adj..max_adj)
            .wrap_err("failed to build chart")?;

        chart
            .configure_mesh()
            .y_label_formatter(&|pp| format!("{pp:.0}pp"))
            .label_style(("sans-serif", 16_i32, &WHITE))
            .bold_line_style(WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw mesh")?;

        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = RGBColor(0, 208, 138).stroke_width(3);
        let iter = (1..).zip(scores).filter_map(|(i, s)| Some((i, s.pp?)));
        let series = AreaSeries::new(iter, 0.0, area_style).border_style(border_style);

        chart
            .draw_series(series)
            .wrap_err("failed to draw area")?
            .label(format!("Max: {max}pp"))
            .legend(EmptyElement::at);

        // Draw empty series for additional label
        let iter = (1..)
            .zip(scores)
            .filter_map(|(i, s)| Some((i, s.pp?)))
            .take(0);

        let series = AreaSeries::new(iter, 0.0, WHITE).border_style(WHITE);

        chart
            .draw_series(series)
            .wrap_err("failed to draw empty series")?
            .label(format!("Min: {min}pp"))
            .legend(EmptyElement::at);

        chart
            .configure_series_labels()
            .border_style(WHITE.mix(0.6).stroke_width(1))
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::UpperRight)
            .legend_area_size(0_i32)
            .label_font(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw legend")?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);

    png_encoder
        .write_image(&buf, W, H, ColorType::Rgb8)
        .wrap_err("failed to encode image")?;

    Ok(png_bytes)
}

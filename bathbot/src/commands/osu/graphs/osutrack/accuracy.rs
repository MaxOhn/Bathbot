use std::iter;

use bathbot_model::ArchivedOsuTrackHistoryEntry;
use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    chart::{ChartBuilder, SeriesLabelPosition},
    prelude::{Circle, IntoDrawingArea},
    series::AreaSeries,
    style::{Color, GREEN, RED, RGBColor, TextStyle, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use skia_safe::{EncodedImageFormat, surfaces};
use time::OffsetDateTime;

use crate::{
    commands::osu::graphs::{H, W},
    util::Monthly,
};

pub(super) fn graph(history: &[ArchivedOsuTrackHistoryEntry]) -> Result<Vec<u8>> {
    let mut min_acc: f32 = 100.0;
    let mut max_acc: f32 = 0.0;

    let mut min_datetime = OffsetDateTime::now_utc();
    let mut max_datetime = OffsetDateTime::now_utc();

    for entry in history {
        let acc = entry.accuracy.to_native();

        if acc < min_acc {
            min_acc = acc;
            min_datetime = entry.timestamp();
        }

        if max_acc < acc {
            max_acc = acc;
            max_datetime = entry.timestamp();
        }
    }

    // The caller already checked that `history` is not empty so indexing here
    // can't panic.
    let start = history[0].timestamp();
    let end = history[history.len() - 1].timestamp();

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let mut root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("Failed to fill background")?;

        let title_style = TextStyle::from(("sans-serif", 25_i32, FontStyle::Bold)).color(&WHITE);
        root = root
            .titled("Accuracy", title_style)
            .wrap_err("Failed to draw title")?;

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(20)
            .y_label_area_size(60)
            .margin(9)
            .build_cartesian_2d(Monthly(start..end), min_acc..max_acc)
            .wrap_err("Failed to build chart")?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(WHITE.mix(0.3))
            .light_line_style(WHITE.mix(0.0)) // hide
            .y_label_formatter(&f32::to_string)
            .label_style(("sans-serif", 20_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw mesh")?;

        let data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.accuracy.to_native()));

        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = RGBColor(0, 208, 138).stroke_width(3);

        let series = AreaSeries::new(data, min_acc, area_style).border_style(border_style);
        chart.draw_series(series).wrap_err("Failed to draw area")?;

        let max_coords = (max_datetime, max_acc);
        let circle = Circle::new(max_coords, 9_u32, GREEN.stroke_width(2));

        chart
            .draw_series(iter::once(circle))
            .wrap_err("Failed to draw max circle")?
            .label(format!("Peak: {max_acc:.3}%"))
            .legend(|(x, y)| Circle::new((x, y), 5_u32, GREEN.stroke_width(2)));

        let min_coords = (min_datetime, min_acc);
        let circle = Circle::new(min_coords, 9_u32, RED.stroke_width(2));

        chart
            .draw_series(iter::once(circle))
            .wrap_err("Failed to draw min circle")?
            .label(format!("Worst: {min_acc:.3}%"))
            .legend(|(x, y)| Circle::new((x, y), 5_u32, RED.stroke_width(2)));

        // Legend
        chart
            .configure_series_labels()
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(15_i32)
            .label_font(("sans-serif", 20_i32, &WHITE))
            .draw()
            .wrap_err("Failed to draw legend")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

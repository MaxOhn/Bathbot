use bathbot_model::ArchivedOsuTrackHistoryEntry;
use bathbot_util::numbers::WithComma;
use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    chart::{ChartBuilder, SeriesLabelPosition},
    prelude::{IntoDrawingArea, PathElement},
    series::LineSeries,
    style::{Color, RGBColor, TextStyle, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use skia_safe::{EncodedImageFormat, surfaces};

use crate::{
    commands::osu::graphs::{H, W},
    util::Monthly,
};

pub(super) fn graph(history: &[ArchivedOsuTrackHistoryEntry]) -> Result<Vec<u8>> {
    // The caller already checked that `history` is not empty so indexing here
    // can't panic.
    let start = history[0].timestamp();
    let end = history[history.len() - 1].timestamp();

    let mut min = i32::MAX;
    let mut max = i32::MIN;

    for entry in history {
        min = min
            .min(entry.count_a.to_native())
            .min(entry.count_s.to_native())
            .min(entry.count_ss.to_native());
        max = max
            .max(entry.count_a.to_native())
            .max(entry.count_s.to_native())
            .max(entry.count_ss.to_native());
    }

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let mut root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("Failed to fill background")?;

        let title_style = TextStyle::from(("sans-serif", 25_i32, FontStyle::Bold)).color(&WHITE);
        root = root
            .titled("Grades", title_style)
            .wrap_err("Failed to draw title")?;

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(20)
            .y_label_area_size(70)
            .margin(9)
            .build_cartesian_2d(Monthly(start..end), min..max)
            .wrap_err("Failed to build chart")?;

        // Mesh and axes
        chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(WHITE.mix(0.3))
            .light_line_style(WHITE.mix(0.0)) // hide
            .y_label_formatter(&|y| WithComma::new(*y).to_string())
            .label_style(("sans-serif", 20_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw mesh")?;

        // Series
        let a_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.count_a.to_native()));

        let a_style = RGBColor(0, 235, 180).stroke_width(2);
        let a_series = LineSeries::new(a_data, a_style);

        chart
            .draw_series(a_series)
            .wrap_err("Failed to draw count_a series")?
            .label("A")
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], a_style));

        let s_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.count_s.to_native()));

        let s_style = RGBColor(0, 116, 193).stroke_width(2);
        let s_series = LineSeries::new(s_data, s_style);

        chart
            .draw_series(s_series)
            .wrap_err("Failed to draw count_s series")?
            .label("S")
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], s_style));

        let ss_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.count_ss.to_native()));

        let ss_style = RGBColor(255, 255, 255).stroke_width(2);
        let ss_series = LineSeries::new(ss_data, ss_style);

        chart
            .draw_series(ss_series)
            .wrap_err("Failed to draw count_ss series")?
            .label("SS")
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], ss_style));

        // Legend
        chart
            .configure_series_labels()
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(40_i32)
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

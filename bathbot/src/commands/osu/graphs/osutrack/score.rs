use bathbot_model::ArchivedOsuTrackHistoryEntry;
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
    let mut min_score = u64::MAX;
    let mut max_score = 0_u64;

    let mut min_level = f32::MAX;
    let mut max_level = 0.0_f32;

    for entry in history {
        min_score = min_score
            .min(entry.total_score.to_native())
            .min(entry.ranked_score.to_native());
        max_score = max_score
            .max(entry.total_score.to_native())
            .max(entry.ranked_score.to_native());

        min_level = min_level.min(entry.level.to_native());
        max_level = max_level.max(entry.level.to_native());
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
            .titled("Total and ranked score", title_style)
            .wrap_err("Failed to draw title")?;

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(20)
            .y_label_area_size(70)
            .right_y_label_area_size(60)
            .margin(9)
            .build_cartesian_2d(Monthly(start..end), min_score..max_score)
            .wrap_err("Failed to build chart")?
            .set_secondary_coord(Monthly(start..end), min_level..max_level);

        // Mesh and axes
        let label_style = ("sans-serif", 20_i32, &WHITE);
        let axis_style = RGBColor(7, 18, 14);
        let axis_desc_style = ("sans-serif", 20_i32, FontStyle::Bold, &WHITE);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(WHITE.mix(0.3))
            .light_line_style(WHITE.mix(0.0)) // hide
            .y_desc("Score")
            .y_label_formatter(&|y| {
                if *y >= 1_000_000_000_000 {
                    format!("{}T", *y as f64 / 1_000_000_000_000.0)
                } else if *y >= 1_000_000_000 {
                    format!("{}B", *y as f64 / 1_000_000_000.0)
                } else if *y >= 1_000_000 {
                    format!("{}M", *y as f64 / 1_000_000.0)
                } else if *y >= 1_000 {
                    format!("{}K", *y as f64 / 1_000.0)
                } else {
                    y.to_string()
                }
            })
            .label_style(label_style)
            .axis_style(axis_style)
            .axis_desc_style(axis_desc_style)
            .draw()
            .wrap_err("Failed to draw mesh")?;

        chart
            .configure_secondary_axes()
            .y_desc("Level")
            .y_label_formatter(&f32::to_string)
            .label_style(label_style)
            .axis_style(axis_style)
            .axis_desc_style(axis_desc_style)
            .draw()
            .wrap_err("Failed to draw secondary mesh")?;

        // Series
        let total_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.total_score.to_native()));

        let total_style = RGBColor(0, 116, 193).stroke_width(2);
        let total_series = LineSeries::new(total_data, total_style);

        chart
            .draw_series(total_series)
            .wrap_err("Failed to draw total score series")?
            .label("Total score")
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], total_style));

        let ranked_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.ranked_score.to_native()));

        let ranked_style = RGBColor(0, 235, 180).stroke_width(2);
        let ranked_series = LineSeries::new(ranked_data, ranked_style);

        chart
            .draw_series(ranked_series)
            .wrap_err("Failed to draw ranked score series")?
            .label("Ranked score")
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], ranked_style));

        let level_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.level.to_native()));

        let level_style = RGBColor(255, 255, 255).stroke_width(2);
        let level_series = LineSeries::new(level_data, level_style);

        chart
            .draw_secondary_series(level_series)
            .wrap_err("Failed to draw level series")?
            .label("Level")
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], level_style));

        // Legend
        chart
            .configure_series_labels()
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(45_i32)
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

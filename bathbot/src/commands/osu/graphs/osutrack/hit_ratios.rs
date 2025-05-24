use bathbot_model::ArchivedOsuTrackHistoryEntry;
use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    chart::ChartBuilder,
    prelude::IntoDrawingArea,
    series::LineSeries,
    style::{Color, RGBColor, TextStyle, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rosu_v2::model::GameMode;
use skia_safe::{EncodedImageFormat, surfaces};

use crate::{
    commands::osu::graphs::{H, W},
    util::Monthly,
};

pub(super) fn graph(mode: GameMode, history: &[ArchivedOsuTrackHistoryEntry]) -> Result<Vec<u8>> {
    let mut min_300: f32 = 100.0;
    let mut max_300: f32 = 0.0;

    let mut min_100: f32 = 100.0;
    let mut max_100: f32 = 0.0;

    let mut min_50: f32 = 100.0;
    let mut max_50: f32 = 0.0;

    for entry in history {
        min_300 = min_300.min(entry.ratio_count300());
        max_300 = max_300.max(entry.ratio_count300());

        min_100 = min_100.min(entry.ratio_count100());
        max_100 = max_100.max(entry.ratio_count100());

        min_50 = min_50.min(entry.ratio_count50());
        max_50 = max_50.max(entry.ratio_count50());
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
            .titled("Hit Ratios", title_style)
            .wrap_err("Failed to draw title")?;

        let split = if mode == GameMode::Taiko { 2 } else { 3 };
        let roots = root.split_evenly((split, 1));

        const MARGIN: i32 = 9;
        const Y_LABEL_AREA_SIZE: i32 = 70;

        let mut chart_300 = ChartBuilder::on(&roots[0])
            .x_label_area_size(15)
            .y_label_area_size(Y_LABEL_AREA_SIZE)
            .margin_top(MARGIN)
            .margin_left(MARGIN)
            .build_cartesian_2d(Monthly(start..end), min_300..max_300)
            .wrap_err("Failed to build first chart")?;

        // Mesh and axes
        chart_300
            .configure_mesh()
            .disable_x_axis()
            .bold_line_style(WHITE.mix(0.3))
            .light_line_style(WHITE.mix(0.0)) // hide
            .y_label_formatter(&f32::to_string)
            .y_desc("% 300")
            .label_style(("sans-serif", 20_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw first mesh")?;

        // Series
        let count300_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.ratio_count300()));

        let count300_style = RGBColor(0, 116, 193).stroke_width(2);
        let count300_series = LineSeries::new(count300_data, count300_style);

        chart_300
            .draw_series(count300_series)
            .wrap_err("Failed to draw first series")?;

        let mut chart_100_builder = ChartBuilder::on(&roots[1]);

        let x_label_area_size_100 = if mode == GameMode::Taiko {
            chart_100_builder.margin_bottom(MARGIN);

            20
        } else {
            15
        };

        let mut chart_100 = chart_100_builder
            .x_label_area_size(x_label_area_size_100)
            .y_label_area_size(Y_LABEL_AREA_SIZE)
            .margin_left(MARGIN)
            .build_cartesian_2d(Monthly(start..end), min_100..max_100)
            .wrap_err("Failed to build second chart")?;

        // Mesh and axes
        let mut chart_100_mesh = chart_100.configure_mesh();

        chart_100_mesh
            .bold_line_style(WHITE.mix(0.3))
            .light_line_style(WHITE.mix(0.0)) // hide
            .y_label_formatter(&f32::to_string)
            .y_desc("% 100")
            .label_style(("sans-serif", 20_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE));

        if mode != GameMode::Taiko {
            chart_100_mesh.disable_x_axis();
        }

        chart_100_mesh
            .draw()
            .wrap_err("Failed to draw second mesh")?;

        // Series
        let count100_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.ratio_count100()));

        let count100_style = RGBColor(0, 235, 180).stroke_width(2);
        let count100_series = LineSeries::new(count100_data, count100_style);

        chart_100
            .draw_series(count100_series)
            .wrap_err("Failed to draw second series")?;

        // Taiko has no 50s
        if mode != GameMode::Taiko {
            let mut chart_50 = ChartBuilder::on(&roots[2])
                .x_label_area_size(20)
                .y_label_area_size(Y_LABEL_AREA_SIZE)
                .margin_bottom(MARGIN)
                .margin_left(MARGIN)
                .build_cartesian_2d(Monthly(start..end), min_50..max_50)
                .wrap_err("Failed to build third chart")?;

            // Mesh and axes
            chart_50
                .configure_mesh()
                .bold_line_style(WHITE.mix(0.3))
                .light_line_style(WHITE.mix(0.0)) // hide
                .y_label_formatter(&f32::to_string)
                .y_desc("% 50")
                .label_style(("sans-serif", 20_i32, &WHITE))
                .axis_style(RGBColor(7, 18, 14))
                .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
                .draw()
                .wrap_err("Failed to draw third mesh")?;

            // Series
            let count50_data = history
                .iter()
                .map(|entry| (entry.timestamp(), entry.ratio_count50()));

            let count50_style = RGBColor(255, 255, 255).stroke_width(2);
            let count50_series = LineSeries::new(count50_data, count50_style);

            chart_50
                .draw_series(count50_series)
                .wrap_err("Failed to draw third series")?;
        }
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

use std::iter;

use bathbot_model::ArchivedOsuTrackHistoryEntry;
use bathbot_util::numbers::WithComma;
use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    chart::{ChartBuilder, SeriesLabelPosition},
    prelude::{Circle, IntoDrawingArea, PathElement},
    series::LineSeries,
    style::{Color, GREEN, RGBColor, TextStyle, WHITE},
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
    let mut min_rank = u32::MAX;
    let mut max_rank = 0_u32;
    let mut min_rank_datetime = OffsetDateTime::now_utc();

    let mut min_pp = f32::MAX;
    let mut max_pp = 0.0_f32;

    for entry in history {
        max_rank = max_rank.max(entry.pp_rank.to_native());

        if min_rank > entry.pp_rank.to_native() {
            min_rank = entry.pp_rank.to_native();
            min_rank_datetime = entry.timestamp();
        }

        min_pp = min_pp.min(entry.pp.to_native());
        max_pp = max_pp.max(entry.pp.to_native());
    }

    let (min_rank, max_rank) = (-(max_rank as i32), -(min_rank as i32));

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
            .titled("Rank and Total PP", title_style)
            .wrap_err("Failed to draw title")?;

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(20)
            .y_label_area_size(90)
            .right_y_label_area_size(90)
            .margin(9)
            .build_cartesian_2d(Monthly(start..end), min_rank..max_rank)
            .wrap_err("Failed to build chart")?
            .set_secondary_coord(Monthly(start..end), min_pp..max_pp);

        // Mesh and axes
        let label_style = ("sans-serif", 20_i32, &WHITE);
        let axis_style = RGBColor(7, 18, 14);
        let axis_desc_style = ("sans-serif", 20_i32, FontStyle::Bold, &WHITE);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(WHITE.mix(0.3))
            .light_line_style(WHITE.mix(0.0)) // hide
            .y_desc("Rank")
            .y_label_formatter(&|y| if *y == 0 { 1 } else { -*y }.to_string())
            .label_style(label_style)
            .axis_style(axis_style)
            .axis_desc_style(axis_desc_style)
            .draw()
            .wrap_err("Failed to draw primary mesh")?;

        chart
            .configure_secondary_axes()
            .y_desc("PP")
            .y_label_formatter(&f32::to_string)
            .label_style(label_style)
            .axis_style(axis_style)
            .axis_desc_style(axis_desc_style)
            .draw()
            .wrap_err("Failed to draw secondary mesh")?;

        // Series
        let rank_data = history
            .iter()
            .map(|entry| (entry.timestamp(), -(entry.pp_rank.to_native() as i32)));

        let rank_style = RGBColor(0, 116, 193).stroke_width(2);
        let rank_series = LineSeries::new(rank_data, rank_style);

        chart
            .draw_series(rank_series)
            .wrap_err("Failed to draw rank series")?
            .label(format!("Rank (peak #{})", WithComma::new(-max_rank as u32)))
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], rank_style));

        let peak_style = GREEN.stroke_width(2);
        let circle = Circle::new((min_rank_datetime, max_rank), 9_u32, peak_style);

        chart
            .draw_series(iter::once(circle))
            .wrap_err("Failed to draw peak circle")?;

        let pp_data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.pp.to_native()));

        let pp_style = RGBColor(0, 246, 193).stroke_width(2);
        let pp_series = LineSeries::new(pp_data, pp_style);

        chart
            .draw_secondary_series(pp_series)
            .wrap_err("Failed to draw pp series")?
            .label("PP")
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], pp_style));

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

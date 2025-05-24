use std::collections::{BTreeMap, btree_map::IntoIter};

use bathbot_model::ArchivedOsuTrackHistoryEntry;
use bathbot_util::numbers::WithComma;
use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    chart::ChartBuilder,
    prelude::{IntoDrawingArea, IntoSegmentedCoord, Rectangle, SegmentValue},
    series::LineSeries,
    style::{Color, RGBColor, TextStyle, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use skia_safe::{EncodedImageFormat, surfaces};
use time::{Date, OffsetDateTime, Time};

use crate::{
    commands::osu::graphs::{H, W},
    util::Monthly,
};

pub(super) fn graph(history: &[ArchivedOsuTrackHistoryEntry]) -> Result<Vec<u8>> {
    // The caller already checked that `history` is not empty so indexing here
    // can't panic.
    let start = history[0].timestamp();
    let end = history[history.len() - 1].timestamp();
    let first_playcount = history[0].playcount.to_native();

    let mut min_playcount = u32::MAX;
    let mut max_playcount = 0_u32;

    let mut year_counts = BTreeMap::<_, YearCountEntry>::new();

    for entry in history {
        let playcount = entry.playcount.to_native();

        min_playcount = min_playcount.min(playcount);
        max_playcount = max_playcount.max(playcount);

        year_counts
            .entry(entry.timestamp().year())
            .or_default()
            .update(playcount);
    }

    let (min_per_year, max_per_year) = year_counts
        .values()
        .scan(first_playcount, |prev_max, entry| {
            let value = entry.max - *prev_max;
            *prev_max = entry.max;

            Some(value)
        })
        .fold((u32::MAX, 0), |(min, max), value| {
            (min.min(value), max.max(value))
        });

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let mut root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("Failed to fill background")?;

        let title_style = TextStyle::from(("sans-serif", 25_i32, FontStyle::Bold)).color(&WHITE);
        root = root
            .titled("Playcount", title_style)
            .wrap_err("Failed to draw title")?;

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(20)
            .y_label_area_size(90)
            .right_y_label_area_size(90)
            .margin(9)
            .build_cartesian_2d(Monthly(start..end), min_playcount..max_playcount)
            .wrap_err("Failed to build chart")?
            .set_secondary_coord(
                Monthly(start..end).into_segmented(),
                min_per_year..max_per_year,
            );

        // Mesh and axes
        let label_style = ("sans-serif", 20_i32, &WHITE);
        let axis_style = RGBColor(7, 18, 14);
        let axis_desc_style = ("sans-serif", 20_i32, FontStyle::Bold, &WHITE);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(WHITE.mix(0.3))
            .light_line_style(WHITE.mix(0.0)) // hide
            .y_desc("Total")
            .y_label_formatter(&|y| WithComma::new(*y).to_string())
            .label_style(label_style)
            .axis_style(axis_style)
            .axis_desc_style(axis_desc_style)
            .draw()
            .wrap_err("Failed to draw primary mesh")?;

        chart
            .configure_secondary_axes()
            .y_desc("Per year")
            .y_label_formatter(&|n| WithComma::new(*n).to_string())
            .label_style(label_style)
            .axis_style(axis_style)
            .axis_desc_style(axis_desc_style)
            .draw()
            .wrap_err("Failed to draw secondary mesh")?;

        // Series
        let bars = YearCountBars::new(year_counts, first_playcount);
        chart
            .draw_secondary_series(bars)
            .wrap_err("Failed to draw bars")?;

        let data = history
            .iter()
            .map(|entry| (entry.timestamp(), entry.playcount.to_native()));

        let series_style = RGBColor(0, 246, 193).stroke_width(2);
        let line = LineSeries::new(data, series_style);

        chart.draw_series(line).wrap_err("Failed to draw series")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

#[derive(Copy, Clone, Debug, Default)]
struct YearCountEntry {
    max: u32,
}

impl YearCountEntry {
    fn update(&mut self, item: u32) {
        self.max = self.max.max(item);
    }
}

struct YearCountBars {
    inner: IntoIter<i32, YearCountEntry>,
    prev_max: u32,
}

impl YearCountBars {
    fn new(counts: BTreeMap<i32, YearCountEntry>, first_playcount: u32) -> Self {
        Self {
            inner: counts.into_iter(),
            prev_max: first_playcount,
        }
    }
}

impl Iterator for YearCountBars {
    type Item = Rectangle<(SegmentValue<OffsetDateTime>, u32)>;

    fn next(&mut self) -> Option<Self::Item> {
        let (year, entry) = self.inner.next()?;
        let value = entry.max - self.prev_max;
        self.prev_max = entry.max;

        let left = SegmentValue::Exact(OffsetDateTime::new_utc(
            Date::from_ordinal_date(year, 1).unwrap(),
            Time::MIDNIGHT,
        ));

        let right = SegmentValue::Exact(OffsetDateTime::new_utc(
            Date::from_ordinal_date(year + 1, 1).unwrap(),
            Time::MIDNIGHT,
        ));

        let top_left = (left, value);
        let bot_right = (right, 0);

        let mix = if value > 0 { 0.5 } else { 0.0 };
        let style = RGBColor(0, 126, 153).mix(mix).filled();

        let mut rect = Rectangle::new([top_left, bot_right], style);
        rect.set_margin(0, 1, 2, 2);

        Some(rect)
    }
}

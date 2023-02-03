use std::fmt::Write;

use eyre::{Result, WrapErr};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::{
    prelude::{
        BitMapBackend, ChartBuilder, Circle, EmptyElement, IntoDrawingArea, IntoSegmentedCoord,
        Rectangle, SegmentValue, SeriesLabelPosition,
    },
    series::PointSeries,
    style::{Color, RGBColor, WHITE},
};
use plotters_backend::FontStyle;
use rosu_v2::prelude::Score;
use time::{OffsetDateTime, UtcOffset};

use crate::commands::osu::graphs::{H, W};

pub async fn top_graph_time(
    mut caption: String,
    scores: &mut [Score],
    tz: UtcOffset,
) -> Result<Vec<u8>> {
    fn date_to_value(date: OffsetDateTime) -> u32 {
        date.hour() as u32 * 60 + date.minute() as u32
    }

    let (h, m, _) = tz.as_hms();
    let _ = write!(caption, " (UTC{h:+})");

    if m != 0 {
        let _ = write!(caption, ":{}", m.abs());
    }

    let mut hours = [0_u8; 24];

    let max = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scores.last().and_then(|s| s.pp).unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    for score in scores.iter_mut() {
        score.ended_at = score.ended_at.to_offset(tz);
        hours[score.ended_at.hour() as usize] += 1;
    }

    scores.sort_unstable_by_key(|s| s.ended_at.time());

    let max_hours = hours.iter().max().map_or(0, |count| *count as u32);

    let len = (W * H) as usize;
    let mut buf = vec![0; len * 3];

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let caption_style = ("sans-serif", 25_i32, FontStyle::Bold, &WHITE);

        let x_label_area_size = 50;
        let y_label_area_size = 60;
        let right_y_label_area_size = 45;
        let margin_bottom = 5;
        let margin_top = 5;
        let margin_right = 15;

        // Draw bars
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .right_y_label_area_size(right_y_label_area_size)
            .margin_bottom(margin_bottom)
            .margin_top(margin_top)
            .margin_right(margin_right)
            .caption(caption, caption_style)
            .build_cartesian_2d((0_u32..23_u32).into_segmented(), 0_u32..max_hours)
            .wrap_err("failed to build bar chart")?
            .set_secondary_coord((0_u32..23_u32).into_segmented(), 0_u32..max_hours);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .disable_y_axis()
            .x_labels(24)
            .x_desc("Hour of the day")
            .label_style(("sans-serif", 16_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw primary bar mesh")?;

        chart
            .configure_secondary_axes()
            .y_desc("#  of  plays  set")
            .label_style(("sans-serif", 16_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw secondary mesh")?;

        let counts = ScoreHourCounts::new(hours);
        chart
            .draw_secondary_series(counts)
            .wrap_err("failed to draw bars")?;

        // Draw points
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .right_y_label_area_size(right_y_label_area_size)
            .margin_bottom(margin_bottom)
            .margin_top(margin_top)
            .margin_right(margin_right)
            .caption("", caption_style)
            .build_cartesian_2d(0_u32..24 * 60, min_adj..max_adj)
            .wrap_err("failed to build point chart")?
            .set_secondary_coord(0_u32..24 * 60, min_adj..max_adj);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_x_axis()
            .y_label_formatter(&|pp| format!("{pp:.0}pp"))
            .x_label_formatter(&|value| format!("{}:{:0>2}", value / 60, value % 60))
            .label_style(("sans-serif", 16_i32, &WHITE))
            .bold_line_style(WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw point mesh")?;

        // Draw secondary axis just to hide its values so that
        // the left hand values aren't displayed instead
        chart
            .configure_secondary_axes()
            .label_style(("", 16_i32, &WHITE.mix(0.0)))
            .axis_style(WHITE.mix(0.0))
            .draw()
            .wrap_err("failed to draw secondary points")?;

        let point_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = WHITE.mix(0.9).stroke_width(1);

        let iter = scores
            .iter()
            .filter_map(|s| Some((date_to_value(s.ended_at), s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, point_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw primary points")?
            .label(format!("Max: {max}pp"))
            .legend(EmptyElement::at);

        let iter = scores
            .iter()
            .filter_map(|s| Some((date_to_value(s.ended_at), s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, border_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw primary points borders")?
            .label(format!("Min: {min}pp"))
            .legend(EmptyElement::at);

        chart
            .configure_series_labels()
            .border_style(WHITE.mix(0.6).stroke_width(1))
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::Coordinate((W as f32 / 4.5) as i32, 10))
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

struct ScoreHourCounts {
    hours: [u8; 24],
    idx: usize,
}

impl ScoreHourCounts {
    fn new(hours: [u8; 24]) -> Self {
        Self { hours, idx: 0 }
    }
}

impl Iterator for ScoreHourCounts {
    type Item = Rectangle<(SegmentValue<u32>, u32)>;

    fn next(&mut self) -> Option<Self::Item> {
        let count = *self.hours.get(self.idx)?;
        let hour = self.idx as u32;
        self.idx += 1;

        let top_left = (SegmentValue::Exact(hour), count as u32);
        let bot_right = (SegmentValue::Exact(hour + 1), 0);

        let mix = if count > 0 { 0.5 } else { 0.0 };
        let style = RGBColor(0, 126, 153).mix(mix).filled();

        let mut rect = Rectangle::new([top_left, bot_right], style);
        rect.set_margin(0, 1, 2, 2);

        Some(rect)
    }
}

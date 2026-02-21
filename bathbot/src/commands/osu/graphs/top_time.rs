use std::{fmt::Write, marker::PhantomData, ops::Range, slice::SliceIndex};

use eyre::{ContextCompat, Result, WrapErr};
use plotters::{
    chart::DualCoordChartContext,
    coord::{
        ranged1d::{DefaultFormatting, KeyPointHint, ValueFormatter},
        types::{RangedCoordf32, RangedCoordu32},
    },
    prelude::{
        Cartesian2d, ChartBuilder, Circle, DiscreteRanged, EmptyElement, IntoDrawingArea,
        IntoSegmentedCoord, Ranged, Rectangle, SegmentValue, SeriesLabelPosition,
    },
    series::PointSeries,
    style::{Color, RGBColor, TextStyle, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rosu_v2::prelude::Score;
use skia_safe::{EncodedImageFormat, Surface, surfaces};
use time::{Duration, OffsetDateTime, UtcOffset};

use crate::commands::osu::graphs::{H, LegendDraw, W};

pub async fn top_graph_time_hour(
    mut caption: String,
    scores: &mut [Score],
    tz: UtcOffset,
) -> Result<Vec<u8>> {
    fn date_to_value(date: OffsetDateTime) -> u32 {
        date.hour() as u32 * 60 + date.minute() as u32
    }

    let (h, m, _) = tz.as_hms();
    let _ = write!(caption, " by hour (UTC{h:+})");

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

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("Failed to fill background")?;

        let caption_style = TextStyle::from(("sans-serif", 25_i32, FontStyle::Bold)).color(&WHITE);

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
            .caption(caption, caption_style.clone())
            .build_cartesian_2d((0_u32..23_u32).into_segmented(), 0_u32..max_hours)
            .wrap_err("Failed to build bar chart")?
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
            .wrap_err("Failed to draw primary bar mesh")?;

        chart
            .configure_secondary_axes()
            .y_desc("#  of  plays  set")
            .label_style(("sans-serif", 16_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw secondary mesh")?;

        let counts = ScoreTimeCounts::<AxisByHour>::new(hours);
        chart
            .draw_secondary_series(counts)
            .wrap_err("Failed to draw bars")?;

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
            .wrap_err("Failed to build point chart")?
            .set_secondary_coord(0_u32..24 * 60, min_adj..max_adj);

        draw_point_mesh(&mut chart)?;

        draw_points(
            &mut chart,
            date_to_value,
            scores,
            max,
            min,
            (W as f32 / 4.5) as i32,
        )?;
    }

    encode_surface(&mut surface)
}

pub async fn top_graph_time_day(
    mut caption: String,
    scores: &mut [Score],
    tz: UtcOffset,
) -> Result<Vec<u8>> {
    fn date_to_value(date: OffsetDateTime) -> u32 {
        date.weekday() as u32 * 24 * 60 + date.hour() as u32 * 60 + date.minute() as u32
    }

    let (h, m, _) = tz.as_hms();
    let _ = write!(caption, " by day (UTC{h:+})");

    if m != 0 {
        let _ = write!(caption, ":{}", m.abs());
    }

    let mut days = [0_u8; 7];

    let max = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scores.last().and_then(|s| s.pp).unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    for score in scores.iter_mut() {
        score.ended_at = score.ended_at.to_offset(tz);
        days[score.ended_at.weekday() as usize] += 1;
    }

    scores.sort_unstable_by_key(|s| s.ended_at.time());

    let max_days = days.iter().max().map_or(0, |count| *count as u32);

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("Failed to fill background")?;

        let caption_style = TextStyle::from(("sans-serif", 25_i32, FontStyle::Bold)).color(&WHITE);

        let x_label_area_size = 35;
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
            .caption(caption, caption_style.clone())
            .build_cartesian_2d(WeekdayRange.into_segmented(), 0_u32..max_days)
            .wrap_err("Failed to build bar chart")?
            .set_secondary_coord(WeekdayRange.into_segmented(), 0_u32..max_days);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .disable_y_axis()
            .x_labels(7)
            .label_style(("sans-serif", 16_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw primary bar mesh")?;

        chart
            .configure_secondary_axes()
            .y_desc("#  of  plays  set")
            .label_style(("sans-serif", 16_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw secondary mesh")?;

        let counts = ScoreTimeCounts::<AxisByDay>::new(days);
        chart
            .draw_secondary_series(counts)
            .wrap_err("Failed to draw bars")?;

        // Draw points
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .right_y_label_area_size(right_y_label_area_size)
            .margin_bottom(margin_bottom)
            .margin_top(margin_top)
            .margin_right(margin_right)
            .caption("", caption_style)
            .build_cartesian_2d(0_u32..7 * 24 * 60, min_adj..max_adj)
            .wrap_err("Failed to build point chart")?
            .set_secondary_coord(0_u32..7 * 24 * 60, min_adj..max_adj);

        draw_point_mesh(&mut chart)?;

        draw_points(
            &mut chart,
            date_to_value,
            scores,
            max,
            min,
            (W as f32 / 6.5) as i32,
        )?;
    }

    encode_surface(&mut surface)
}

fn encode_surface(surface: &mut Surface) -> Result<Vec<u8>> {
    surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .map(|data| data.to_vec())
        .wrap_err("Failed to encode image")
}

type Chart<'a> = DualCoordChartContext<
    'a,
    SkiaBackend<'a>,
    Cartesian2d<RangedCoordu32, RangedCoordf32>,
    Cartesian2d<RangedCoordu32, RangedCoordf32>,
>;

fn draw_point_mesh(chart: &mut Chart<'_>) -> Result<()> {
    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_x_axis()
        .y_label_formatter(&|pp| format!("{pp:.0}pp"))
        .label_style(("sans-serif", 16_i32, &WHITE))
        .bold_line_style(WHITE.mix(0.3))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
        .draw()
        .wrap_err("Failed to draw point mesh")
}

fn draw_points(
    chart: &mut Chart,
    date_to_value: fn(OffsetDateTime) -> u32,
    scores: &[Score],
    max_pp: f32,
    min_pp: f32,
    legend_pos: i32,
) -> Result<()> {
    // Draw secondary axis just to hide its values so that
    // the left hand values aren't displayed instead
    chart
        .configure_secondary_axes()
        .label_style(("", 16_i32, &WHITE.mix(0.0)))
        .axis_style(WHITE.mix(0.0))
        .draw()
        .wrap_err("Failed to draw secondary points")?;

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
        .wrap_err("Failed to draw primary points")?
        .label(format!("Max: {max_pp}pp"))
        .legend(EmptyElement::at);

    let iter = scores
        .iter()
        .filter_map(|s| Some((date_to_value(s.ended_at), s.pp?)));

    let series = PointSeries::of_element(iter, 3_i32, border_style, &|coord, size, style| {
        EmptyElement::at(coord) + Circle::new((0, 0), size, style)
    });

    chart
        .draw_series(series)
        .wrap_err("Failed to draw primary points borders")?
        .label(format!("Min: {min_pp}pp"))
        .legend(EmptyElement::at);

    LegendDraw::new(&mut *chart)
        .position(SeriesLabelPosition::Coordinate(legend_pos, 10))
        .draw()?;

    Ok(())
}

trait TimeAxis {
    type Counts: TryGet<usize, Output = u8>;
    type Value: Copy;

    fn value_from_idx(idx: usize) -> Self::Value;

    fn right_segment(value: Self::Value) -> SegmentValue<Self::Value>;
}

trait TryGet<I> {
    type Output;

    fn try_get(&self, idx: I) -> Option<&<Self as TryGet<I>>::Output>;
}

impl<T, I, const N: usize> TryGet<I> for [T; N]
where
    I: SliceIndex<[T], Output: Sized>,
{
    type Output = <I as SliceIndex<[T]>>::Output;

    fn try_get(&self, idx: I) -> Option<&<Self as TryGet<I>>::Output> {
        self.get(idx)
    }
}

struct AxisByHour;

impl TimeAxis for AxisByHour {
    type Counts = [u8; 24];
    type Value = u32;

    fn value_from_idx(idx: usize) -> Self::Value {
        idx as u32
    }

    fn right_segment(value: Self::Value) -> SegmentValue<Self::Value> {
        SegmentValue::Exact(value + 1)
    }
}

struct AxisByDay;

impl TimeAxis for AxisByDay {
    type Counts = [u8; 7];
    type Value = Weekday;

    fn value_from_idx(idx: usize) -> Self::Value {
        Weekday::from(idx as u8)
    }

    fn right_segment(value: Self::Value) -> SegmentValue<Self::Value> {
        value.next().map_or(SegmentValue::Last, SegmentValue::Exact)
    }
}

struct ScoreTimeCounts<A: TimeAxis> {
    counts: A::Counts,
    idx: usize,
    axis: PhantomData<A>,
}

impl<A: TimeAxis> ScoreTimeCounts<A> {
    fn new(counts: A::Counts) -> Self {
        Self {
            counts,
            idx: 0,
            axis: PhantomData,
        }
    }
}

impl<A: TimeAxis> Iterator for ScoreTimeCounts<A> {
    type Item = Rectangle<(SegmentValue<A::Value>, u32)>;

    fn next(&mut self) -> Option<Self::Item> {
        let count = *self.counts.try_get(self.idx)?;
        let value = A::value_from_idx(self.idx);
        self.idx += 1;

        let top_left = (SegmentValue::Exact(value), count as u32);
        let bot_right = (A::right_segment(value), 0);

        let mix = if count > 0 { 0.5 } else { 0.0 };
        let style = RGBColor(0, 126, 153).mix(mix).filled();

        let mut rect = Rectangle::new([top_left, bot_right], style);
        rect.set_margin(0, 1, 2, 2);

        Some(rect)
    }
}

#[derive(Copy, Clone)]
enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    const fn next(self) -> Option<Self> {
        match self {
            Self::Monday => Some(Self::Tuesday),
            Self::Tuesday => Some(Self::Wednesday),
            Self::Wednesday => Some(Self::Thursday),
            Self::Thursday => Some(Self::Friday),
            Self::Friday => Some(Self::Saturday),
            Self::Saturday => Some(Self::Sunday),
            Self::Sunday => None,
        }
    }
}

impl From<u8> for Weekday {
    fn from(day: u8) -> Self {
        match day {
            0 => Self::Monday,
            1 => Self::Tuesday,
            2 => Self::Wednesday,
            3 => Self::Thursday,
            4 => Self::Friday,
            5 => Self::Saturday,
            6 => Self::Sunday,
            _ => panic!("bad day index"),
        }
    }
}

struct WeekdayRange;

impl Ranged for WeekdayRange {
    type FormatOption = DefaultFormatting;
    type ValueType = Weekday;

    fn map(&self, value: &Self::ValueType, (min, max): (i32, i32)) -> i32 {
        let total_ns = Duration::days(6).whole_nanoseconds();
        let value_ns = Duration::days(*value as i64).whole_nanoseconds();

        ((max - min) as f64 * value_ns as f64 / total_ns as f64) as i32 + min
    }

    fn key_points<Hint: KeyPointHint>(&self, _: Hint) -> Vec<Self::ValueType> {
        vec![
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
            Weekday::Sunday,
        ]
    }

    fn range(&self) -> Range<Self::ValueType> {
        Weekday::Monday..Weekday::Sunday
    }
}

impl DiscreteRanged for WeekdayRange {
    fn size(&self) -> usize {
        7
    }

    fn index_of(&self, value: &Self::ValueType) -> Option<usize> {
        Some(*value as usize)
    }

    fn from_index(&self, index: usize) -> Option<Self::ValueType> {
        Some(Weekday::from(index as u8))
    }
}

impl ValueFormatter<Weekday> for WeekdayRange {
    fn format(day: &Weekday) -> String {
        let s = match day {
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
            Weekday::Sunday => "Sunday",
        };

        s.to_owned()
    }
}

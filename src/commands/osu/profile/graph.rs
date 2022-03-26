use std::{iter, mem};

use chrono::{Datelike, TimeZone, Utc};
use futures::stream::{FuturesUnordered, TryStreamExt};
use image::{
    codecs::png::PngEncoder, imageops::FilterType::Lanczos3, load_from_memory, ColorType,
    ImageEncoder,
};
use plotters::prelude::*;
use rosu_v2::prelude::{MonthlyCount, User};

use crate::{core::Context, error::GraphError};

pub async fn graphs(
    ctx: &Context,
    user: &mut User,
    w: u32,
    h: u32,
) -> Result<Option<Vec<u8>>, GraphError> {
    let mut monthly_playcount = mem::replace(&mut user.monthly_playcounts, None).unwrap();
    let badges = mem::replace(&mut user.badges, None).unwrap();

    if monthly_playcount.len() < 2 {
        return Ok(None);
    }

    let len = (w * h) as usize;
    let mut buf = vec![0; len * 3]; // PIXEL_SIZE = 3

    {
        // Request all badge images
        let badges = match badges.is_empty() {
            true => Vec::new(),
            false => {
                badges
                    .iter()
                    .map(|badge| ctx.clients.custom.get_badge(&badge.image_url))
                    .collect::<FuturesUnordered<_>>()
                    .try_collect()
                    .await?
            }
        };

        // Setup total canvas
        let root = BitMapBackend::with_buffer(&mut buf, (w, h)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)?;

        // Draw badges if there are any
        let canvas = if badges.is_empty() {
            root
        } else {
            let max_badges_per_row = 10;
            let margin = 5;
            let inner_margin = 3;
            let badge_count = badges.len() as u32;
            let badge_rows = ((badge_count - 1) / max_badges_per_row) + 1;
            let badge_total_height = (badge_rows * 60).min(h / 2);
            let badge_height = badge_total_height / badge_rows;
            let (top, bottom) = root.split_vertically(badge_total_height);
            let mut rows = Vec::with_capacity(badge_rows as usize);
            let mut last = top;

            for _ in 0..badge_rows {
                let (curr, remain) = last.split_vertically(badge_height);
                rows.push(curr);
                last = remain;
            }

            let badge_width =
                (w - 2 * margin - (max_badges_per_row - 1) * inner_margin) / max_badges_per_row;

            // Draw each row of badges
            for (row, chunk) in badges.chunks(max_badges_per_row as usize).enumerate() {
                let x_offset = (max_badges_per_row - chunk.len() as u32) * badge_width / 2;

                let mut chart_row = ChartBuilder::on(&rows[row])
                    .margin(margin)
                    .build_cartesian_2d(0..w, 0..badge_height)?;

                chart_row
                    .configure_mesh()
                    .disable_x_axis()
                    .disable_y_axis()
                    .disable_x_mesh()
                    .disable_y_mesh()
                    .draw()?;

                for (idx, badge) in chunk.iter().enumerate() {
                    let badge_img =
                        load_from_memory(badge)?.resize_exact(badge_width, badge_height, Lanczos3);

                    let x = x_offset + idx as u32 * badge_width + idx as u32 * inner_margin;
                    let y = badge_height;
                    let elem: BitMapElement<'_, _> = ((x, y), badge_img).into();
                    chart_row.draw_series(iter::once(elem))?;
                }
            }

            bottom
        };

        let replays = user.replays_watched_counts.as_mut().unwrap();

        // Spoof missing months
        spoof_monthly_counts(&mut monthly_playcount);

        // Spoof missing replays
        let dates = monthly_playcount
            .iter()
            .map(|date_count| date_count.start_date)
            .enumerate();

        for (i, start_date) in dates {
            let cond = replays.get(i).map(|c| c.start_date == start_date);

            if cond != Some(true) {
                let count = MonthlyCount {
                    start_date,
                    count: 0,
                };

                replays.insert(i, count);
            }
        }

        let left_first = monthly_playcount.first().unwrap().start_date;
        let left_last = monthly_playcount.last().unwrap().start_date;

        let left_max = monthly_playcount
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap();

        let right_first = replays.first().unwrap().start_date;
        let right_last = replays.last().unwrap().start_date;

        let right_max = replays
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap()
            .max(1);

        let right_label_area: i32 = match right_max {
            n if n < 10 => 40,
            n if n < 100 => 50,
            n if n < 1000 => 60,
            n if n < 10_000 => 70,
            n if n < 100_000 => 80,
            _ => 90,
        };

        let mut chart = ChartBuilder::on(&canvas)
            .margin(9_i32)
            .x_label_area_size(20_i32)
            .y_label_area_size(75_i32)
            .right_y_label_area_size(right_label_area)
            .build_cartesian_2d((left_first..left_last).monthly(), 0..left_max)?
            .set_secondary_coord((right_first..right_last).monthly(), 0..right_max);

        // Mesh and labels
        chart
            .configure_mesh()
            .light_line_style(&BLACK.mix(0.0))
            .disable_x_mesh()
            .x_labels(10)
            .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month()))
            .y_desc("Monthly playcount")
            .label_style(("sans-serif", 20_i32, &WHITE))
            .bold_line_style(&WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
            .draw()?;

        chart
            .configure_secondary_axes()
            .y_desc("Replays watched")
            .label_style(("sans-serif", 20_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
            .draw()?;

        // Draw playcount area
        let iter = monthly_playcount
            .iter()
            .map(|MonthlyCount { start_date, count }| (*start_date, *count));

        let area_color = RGBColor(0, 116, 193);
        let border_color = RGBColor(102, 174, 222);
        let series = AreaSeries::new(iter, 0, area_color.mix(0.5).filled());

        chart
            .draw_series(series.border_style(border_color.stroke_width(1)))?
            .label("Monthly playcount")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], area_color.stroke_width(2))
            });

        // Draw circles
        let circles = monthly_playcount
            .iter()
            .map(move |MonthlyCount { start_date, count }| {
                let style = border_color.mix(0.6).filled();

                Circle::new((*start_date, *count), 2_i32, style)
            });

        chart.draw_series(circles)?;

        // Draw replay watched area
        let iter = replays
            .iter()
            .map(|MonthlyCount { start_date, count }| (*start_date, *count));

        let area_color = RGBColor(0, 246, 193);
        let border_color = RGBColor(40, 246, 205);
        let series = AreaSeries::new(iter, 0, area_color.mix(0.2).filled());

        chart
            .draw_secondary_series(series.border_style(border_color.stroke_width(1)))?
            .label("Replays watched")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], border_color.stroke_width(2))
            });

        // Draw circles
        let circles = replays.iter().map(|MonthlyCount { start_date, count }| {
            let style = border_color.stroke_width(1).filled();

            Circle::new((*start_date, *count), 2_i32, style)
        });

        chart.draw_secondary_series(circles)?;

        // Legend
        chart
            .configure_series_labels()
            .background_style(&RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(45_i32)
            .label_font(("sans-serif", 20_i32, &WHITE))
            .draw()?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, w, h, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}

fn spoof_monthly_counts(counts: &mut Vec<MonthlyCount>) {
    let (mut year, mut month) = match counts.as_slice() {
        [] | [_] => return,
        [first, ..] => (first.start_date.year(), first.start_date.month()),
    };

    let mut i = 1;

    while i < counts.len() {
        month += 1;

        if month == 13 {
            month = 1;
            year += 1;
        }

        let date = Utc.ymd(year, month, 1);

        if date < counts[i].start_date {
            let count = MonthlyCount {
                start_date: date,
                count: 0,
            };

            counts.insert(i, count);
        }

        i += 1;
    }
}

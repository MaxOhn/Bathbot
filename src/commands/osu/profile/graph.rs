use std::mem;

use chrono::Datelike;
use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use image::{
    codecs::png::PngEncoder, imageops::FilterType::Lanczos3, load_from_memory, ColorType,
    ImageEncoder,
};
use plotters::prelude::*;
use reqwest::{Client as ReqwestClient, Response};
use rosu_v2::prelude::{MonthlyCount, User};

use crate::error::GraphError;

const W: u32 = 1350;
const H: u32 = 350;

pub(super) async fn graphs(user: &mut User) -> Result<Option<Vec<u8>>, GraphError> {
    let mut monthly_playcount = mem::replace(&mut user.monthly_playcounts, None).unwrap();
    let badges = mem::replace(&mut user.badges, None).unwrap();

    if monthly_playcount.len() < 2 {
        return Ok(None);
    }

    static LEN: usize = W as usize * H as usize;
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3

    {
        // Request all badge images
        let badges = match badges.is_empty() {
            true => Vec::new(),
            false => {
                let client = ReqwestClient::new();

                badges
                    .iter()
                    .map(|badge| {
                        client
                            .get(&badge.image_url)
                            .send()
                            .and_then(Response::bytes)
                            .map_ok(|bytes| bytes.to_vec())
                    })
                    .collect::<FuturesUnordered<_>>()
                    .try_collect()
                    .await?
            }
        };

        // Setup total canvas
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;

        // Draw badges if there are any
        let canvas = if badges.is_empty() {
            root
        } else {
            let max_badges_per_row = 10;
            let margin = 5;
            let inner_margin = 3;
            let badge_count = badges.len() as u32;
            let badge_rows = ((badge_count - 1) / max_badges_per_row) + 1;
            let badge_total_height = (badge_rows * 60).min(H / 2);
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
                (W - 2 * margin - (max_badges_per_row - 1) * inner_margin) / max_badges_per_row;

            // Draw each row of badges
            for (row, chunk) in badges.chunks(max_badges_per_row as usize).enumerate() {
                let x_offset = (max_badges_per_row - chunk.len() as u32) * badge_width / 2;

                let mut chart_row = ChartBuilder::on(&rows[row])
                    .margin(margin)
                    .build_cartesian_2d(0..W, 0..badge_height)?;

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
                    chart_row.draw_series(std::iter::once(elem))?;
                }
            }

            bottom
        };

        let replays = user.replays_watched_counts.as_mut().unwrap();

        // Spoof missing months
        // Making use of the fact that the dates are always of the form YYYY-MM-01
        let first_date = monthly_playcount.first().unwrap().start_date;
        let mut curr_month = first_date.month();
        let mut curr_year = first_date.year();

        let dates = monthly_playcount
            .iter()
            .map(|date_count| date_count.start_date)
            .enumerate()
            .collect::<Vec<_>>()
            .into_iter();

        let mut inserted = 0;

        for (i, date) in dates {
            while date.month() != curr_month || date.year() != curr_year {
                let spoofed_date = date
                    .with_month(curr_month)
                    .unwrap()
                    .with_year(curr_year)
                    .unwrap();

                let count = MonthlyCount {
                    start_date: spoofed_date,
                    count: 0,
                };

                monthly_playcount.insert(inserted + i, count);
                inserted += 1;
                curr_month += 1;

                if curr_month == 13 {
                    curr_month = 1;
                    curr_year += 1;
                }
            }

            curr_month += 1;

            if curr_month == 13 {
                curr_month = 1;
                curr_year += 1;
            }
        }

        // Spoof missing replays
        let dates = monthly_playcount
            .iter()
            .map(|date_count| date_count.start_date)
            .enumerate();

        for (i, date) in dates {
            let cond = replays
                .get(i)
                .map(|date_count| date_count.start_date == date);

            let count = MonthlyCount {
                start_date: date,
                count: 0,
            };

            if let None | Some(false) = cond {
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
            .label_style(("sans-serif", 20_i32))
            .draw()?;

        chart
            .configure_secondary_axes()
            .y_desc("Replays watched")
            .label_style(("sans-serif", 20_i32))
            .draw()?;

        // Draw playcount area
        chart
            .draw_series(
                AreaSeries::new(
                    monthly_playcount
                        .iter()
                        .map(|MonthlyCount { start_date, count }| (*start_date, *count)),
                    0,
                    &BLUE.mix(0.2),
                )
                .border_style(&BLUE),
            )?
            .label("Monthly playcount")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE.stroke_width(2)));

        // Draw circles
        chart.draw_series(
            monthly_playcount
                .iter()
                .map(|MonthlyCount { start_date, count }| {
                    Circle::new((*start_date, *count), 2_i32, BLUE.filled())
                }),
        )?;

        // Draw replay watched area
        chart
            .draw_secondary_series(
                AreaSeries::new(
                    replays
                        .iter()
                        .map(|MonthlyCount { start_date, count }| (*start_date, *count)),
                    0,
                    &RED.mix(0.2),
                )
                .border_style(&RED),
            )?
            .label("Replays watched")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED.stroke_width(2)));

        // Draw circles
        chart.draw_secondary_series(replays.iter().map(|MonthlyCount { start_date, count }| {
            Circle::new((*start_date, *count), 2_i32, RED.filled())
        }))?;

        // Legend
        chart
            .configure_series_labels()
            .background_style(&RGBColor(192, 192, 192))
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(45_i32)
            .label_font(("sans-serif", 20_i32))
            .draw()?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, W, H, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}

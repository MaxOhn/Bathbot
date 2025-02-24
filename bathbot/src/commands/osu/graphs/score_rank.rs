use std::iter;

use bathbot_model::RespektiveUser;
use bathbot_util::{AuthorBuilder, constants::GENERAL_ISSUE, numbers::WithComma};
use eyre::{ContextCompat, Report, Result, WrapErr};
use plotters::{
    prelude::{ChartBuilder, Circle, IntoDrawingArea, SeriesLabelPosition},
    series::AreaSeries,
    style::{BLACK, Color, GREEN, RED, RGBColor, ShapeStyle, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use skia_safe::{EncodedImageFormat, surfaces};
use time::OffsetDateTime;

use crate::{
    commands::osu::{
        graphs::{H, W},
        rank, user_not_found,
    },
    core::{Context, commands::CommandOrigin},
    manager::redis::osu::{UserArgs, UserArgsError},
};

pub async fn score_rank_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
    mode: GameMode,
) -> Result<Option<(AuthorBuilder, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;
            orig.error(content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let users_fut =
        Context::client().get_respektive_users(iter::once(user.user_id.to_native()), mode);

    let respektive_user = match users_fut.await {
        Ok(mut users) => users.next().flatten(),
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get user"));
        }
    };

    let bytes = match draw_graph(respektive_user.as_ref()) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!(
                "`{name}` has no available rank data :(",
                name = user.username.as_str()
            );

            orig.error(content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            warn!(?err, "Failed to draw rank graph");

            return Ok(None);
        }
    };

    let author = rank::author(&user, respektive_user.as_ref());

    Ok(Some((author, bytes)))
}

fn draw_graph(user: Option<&RespektiveUser>) -> Result<Option<Vec<u8>>> {
    let Some(user) = user else { return Ok(None) };
    let Some(ref rank_history) = user.rank_history else {
        return Ok(None);
    };

    if rank_history.is_empty() {
        return Ok(None);
    }

    let history_len = rank_history.len();

    let mut min = u32::MAX;
    let mut max = 0;

    let mut min_idx = 0;
    let mut max_idx = 0;

    for (i, entry) in rank_history.iter().rev().enumerate() {
        let Some(rank) = entry.rank else { continue };

        if rank == 0 {
            continue;
        }

        if rank < min {
            min = rank;
            min_idx = i;

            if rank > max {
                max = rank;
                max_idx = i;
            }
        } else if rank > max {
            max = rank;
            max_idx = i;
        }
    }

    let y_label_area_size = if max > 1_000_000 {
        85
    } else if max > 100_000 {
        80
    } else if max > 10_000 {
        75
    } else if max > 1000 {
        70
    } else if max > 100 {
        65
    } else if max > 10 {
        60
    } else {
        50
    };

    let (min, max) = (-(max as i32), -(min as i32));

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("Failed to fill background")?;

        let style: fn(RGBColor) -> ShapeStyle = |color| ShapeStyle {
            color: color.to_rgba(),
            filled: false,
            stroke_width: 1,
        };

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(40)
            .y_label_area_size(y_label_area_size)
            .margin(10)
            .margin_left(6)
            .build_cartesian_2d(0_i32..history_len.saturating_sub(1) as i32, min..max)
            .wrap_err("Failed to build chart")?;

        let now = OffsetDateTime::now_utc();

        let oldest = rank_history
            .last()
            .map(|entry| (now - entry.date).whole_days() as i32)
            .unwrap_or(90);

        chart
            .configure_mesh()
            .disable_y_mesh()
            .x_labels(20)
            .x_desc("Days ago")
            .x_label_formatter(&|x| format!("{}", oldest - *x))
            .y_label_formatter(&|y| format!("{}", -*y))
            .y_desc("Rank")
            .label_style(("sans-serif", 15, &WHITE))
            .bold_line_style(WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw mesh")?;

        let data = rank_history.iter().filter_map(|entry| {
            let rank = entry.rank?;
            let whole_days = (now - entry.date).whole_days() as i32;

            Some((oldest - whole_days, -(rank as i32)))
        });

        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = style(RGBColor(0, 208, 138)).stroke_width(3);
        let series = AreaSeries::new(data, min, area_style).border_style(border_style);
        chart.draw_series(series).wrap_err("Failed to draw area")?;

        let max_coords = (min_idx as i32, max);
        let circle = Circle::new(max_coords, 9_i32, style(GREEN).stroke_width(2));

        chart
            .draw_series(iter::once(circle))
            .wrap_err("Failed to draw max circle")?
            .label(format!("Peak: #{}", WithComma::new(-max)))
            .legend(|(x, y)| Circle::new((x, y), 5_i32, style(GREEN).stroke_width(2)));

        let min_coords = (max_idx as i32, min);
        let circle = Circle::new(min_coords, 9_i32, style(RED).stroke_width(2));

        chart
            .draw_series(iter::once(circle))
            .wrap_err("Failed to draw min circle")?
            .label(format!("Worst: #{}", WithComma::new(-min)))
            .legend(|(x, y)| Circle::new((x, y), 5_i32, style(RED).stroke_width(2)));

        let position = if min_idx <= 70 {
            SeriesLabelPosition::UpperRight
        } else if max_idx > 70 {
            SeriesLabelPosition::UpperLeft
        } else {
            SeriesLabelPosition::LowerRight
        };

        chart
            .configure_series_labels()
            .border_style(BLACK.stroke_width(2))
            .background_style(RGBColor(192, 192, 192))
            .position(position)
            .legend_area_size(13)
            .label_font(("sans-serif", 15, FontStyle::Bold))
            .draw()
            .wrap_err("Failed to draw legend")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(Some(png_bytes))
}

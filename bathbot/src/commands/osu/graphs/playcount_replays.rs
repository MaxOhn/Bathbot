use std::iter;

use bathbot_macros::command;
use bathbot_model::rosu_v2::user::MonthlyCountRkyv;
use bathbot_util::{MessageBuilder, constants::GENERAL_ISSUE, matcher};
use bitflags::bitflags;
use bytes::Bytes;
use eyre::{ContextCompat, Report, Result, WrapErr};
use futures::{TryStreamExt, stream::FuturesUnordered};
use image::imageops::FilterType::Lanczos3;
use plotters::{
    coord::{Shift, types::RangedCoordi32},
    prelude::{
        Cartesian2d, ChartBuilder, ChartContext, Circle, DrawingArea, IntoDrawingArea, PathElement,
        SeriesLabelPosition,
    },
    series::AreaSeries,
    style::{BLACK, Color, RGBColor, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rkyv::{
    rancor::{Panic, ResultExt},
    with::{Map, With},
};
use rosu_v2::{
    model::GameMode,
    prelude::{MonthlyCount, OsuError},
    request::UserId,
};
use skia_safe::{EncodedImageFormat, Surface, surfaces};
use time::{Date, Month, OffsetDateTime};
use twilight_model::guild::Permissions;

use super::{BitMapElement, Graph, GraphPlaycountReplays, H, W};
use crate::{
    commands::osu::{graphs::GRAPH_PLAYCOUNT_DESC, user_not_found},
    core::{
        Context,
        commands::{CommandOrigin, prefix::Args},
    },
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::Monthly,
};

impl<'m> GraphPlaycountReplays<'m> {
    fn args(args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;

        for arg in args {
            if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            name,
            discord,
            playcount: None,
            replays: None,
            badges: None,
        }
    }
}

#[command]
#[desc(GRAPH_PLAYCOUNT_DESC)]
#[usage("[username]")]
#[examples("peppy")]
#[group(AllModes)]
async fn prefix_graphplaycount(
    msg: &Message,
    args: Args<'_>,
    perms: Option<Permissions>,
) -> Result<()> {
    let args = GraphPlaycountReplays::args(args);
    let orig = CommandOrigin::from_msg(msg, perms);

    super::graph(orig, Graph::PlaycountReplays(args)).await
}

pub async fn playcount_replays_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
    flags: ProfileGraphFlags,
) -> Result<Option<(CachedUser, Vec<u8>)>> {
    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let mut user = match Context::redis().osu_user(user_args).await {
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

    let params = ProfileGraphParams::new(&mut user)
        .width(W)
        .height(H)
        .flags(flags);

    let bytes = match graphs(params).await {
        Ok(GraphResult::Ok(graph)) => graph,
        Ok(GraphResult::NotEnoughDatapoints) => {
            let content = format!(
                "`{}` does not have enough playcount data points",
                user.username.as_str()
            );

            let builder = MessageBuilder::new().embed(content);
            orig.create_message(builder).await?;

            return Ok(None);
        }
        Ok(GraphResult::NoBadges) => {
            let content = format!("`{}` does not have any badges", user.username.as_str());
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            warn!(?err, "Failed to create profile graph");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

bitflags! {
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct ProfileGraphFlags: u8 {
        const BADGES    = 1 << 0;
        const PLAYCOUNT = 1 << 1;
        const REPLAYS   = 1 << 2;
    }
}

impl ProfileGraphFlags {
    fn badges(self) -> bool {
        self.contains(Self::BADGES)
    }

    fn playcount(self) -> bool {
        self.contains(Self::PLAYCOUNT)
    }

    fn replays(self) -> bool {
        self.contains(Self::REPLAYS)
    }
}

impl Default for ProfileGraphFlags {
    #[inline]
    fn default() -> Self {
        Self::all()
    }
}

pub struct ProfileGraphParams<'l> {
    user: &'l mut CachedUser,
    w: u32,
    h: u32,
    flags: ProfileGraphFlags,
}

impl<'l> ProfileGraphParams<'l> {
    const H: u32 = 350;
    const W: u32 = 1350;

    pub fn new(user: &'l mut CachedUser) -> Self {
        Self {
            user,
            w: Self::W,
            h: Self::H,
            flags: ProfileGraphFlags::default(),
        }
    }

    pub fn width(mut self, w: u32) -> Self {
        self.w = w;

        self
    }

    pub fn height(mut self, h: u32) -> Self {
        self.h = h;

        self
    }

    pub fn flags(mut self, flags: ProfileGraphFlags) -> Self {
        self.flags = flags;

        self
    }
}

type Area<'b> = DrawingArea<SkiaBackend<'b>, Shift>;
type Chart<'a, 'b> = ChartContext<'a, SkiaBackend<'b>, Cartesian2d<Monthly<Date>, RangedCoordi32>>;

// Request all badge images if required
async fn gather_badges(user: &mut CachedUser, flags: ProfileGraphFlags) -> Result<Vec<Bytes>> {
    let badges = user.badges.as_slice();

    if badges.is_empty() || !flags.badges() {
        return Ok(Vec::new());
    }

    let client = Context::client();

    badges
        .iter()
        .map(|badge| client.get_badge(&badge.image_url))
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await
}

async fn graphs(params: ProfileGraphParams<'_>) -> Result<GraphResult> {
    let w = params.w;
    let h = params.h;

    let badges = gather_badges(params.user, params.flags)
        .await
        .wrap_err("Failed to gather badges")?;

    let mut surface =
        surfaces::raster_n32_premul((w as i32, h as i32)).wrap_err("Failed to create surface")?;

    if params.flags == ProfileGraphFlags::BADGES && badges.is_empty() {
        return Ok(GraphResult::NoBadges);
    } else if !draw(&mut surface, params, &badges)? {
        return Ok(GraphResult::NotEnoughDatapoints);
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(GraphResult::Ok(png_bytes))
}

fn draw(surface: &mut Surface, params: ProfileGraphParams<'_>, badges: &[Bytes]) -> Result<bool> {
    let ProfileGraphParams { user, w, h, flags } = params;

    let (playcounts, replays) = prepare_monthly_counts(user, flags);

    if (flags.playcount() && playcounts.len() < 2) || (!flags.playcount() && replays.len() < 2) {
        return Ok(false);
    }

    let canvas = if flags.badges() && !badges.is_empty() {
        // Needs to happen after .await since type is not Send
        let root = create_root(surface, w, h)?;

        draw_badges(badges, root, w, h)?
    } else {
        create_root(surface, w, h)?
    };

    if flags.playcount() && flags.replays() {
        draw_both(&playcounts, &replays, &canvas)?;
    } else if flags.replays() {
        draw_replays(&replays, &canvas)?;
    } else if flags.playcount() {
        draw_playcounts(&playcounts, &canvas)?;
    }

    Ok(true)
}

fn create_root(surface: &mut Surface, w: u32, h: u32) -> Result<Area<'_>> {
    let root = SkiaBackend::new(surface.canvas(), w, h).into_drawing_area();

    let background = RGBColor(19, 43, 33);
    root.fill(&background)
        .wrap_err("Failed to fill background")?;

    Ok(root)
}

fn draw_badges<'a>(badges: &[Bytes], area: Area<'a>, w: u32, h: u32) -> Result<Area<'a>> {
    let max_badges_per_row = 10;
    let margin = 5;
    let inner_margin = 3;

    let badge_count = badges.len() as u32;
    let badge_rows = ((badge_count - 1) / max_badges_per_row) + 1;
    let badge_total_height = (badge_rows * 60).min(h / 2);
    let badge_height = badge_total_height / badge_rows;

    let (top, bottom) = area.split_vertically(badge_total_height);

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
            .build_cartesian_2d(0..w, 0..badge_height)
            .wrap_err("Failed to build badge chart")?;

        chart_row
            .configure_mesh()
            .disable_x_axis()
            .disable_y_axis()
            .disable_x_mesh()
            .disable_y_mesh()
            .draw()
            .wrap_err("Failed to draw badge mesh")?;

        for (idx, badge) in chunk.iter().enumerate() {
            let badge_img = image::load_from_memory(badge)
                .wrap_err("Failed to get badge from memory")?
                .resize_exact(badge_width, badge_height, Lanczos3);

            let x = x_offset + idx as u32 * badge_width + idx as u32 * inner_margin;
            let y = badge_height;

            let elem = BitMapElement::new(badge_img, (x, y));

            chart_row
                .draw_series(iter::once(elem))
                .wrap_err("Failed to draw badge")?;
        }
    }

    Ok(bottom)
}

const PLAYCOUNTS_AREA_COLOR: RGBColor = RGBColor(0, 116, 193);
const PLAYCOUNTS_BORDER_COLOR: RGBColor = RGBColor(102, 174, 222);

fn draw_playcounts(playcounts: &[MonthlyCount], canvas: &Area<'_>) -> Result<()> {
    let (first, last, max) = first_last_max(playcounts);

    let mut chart = ChartBuilder::on(canvas)
        .margin(9_i32)
        .x_label_area_size(20_i32)
        .y_label_area_size(75_i32)
        .build_cartesian_2d(Monthly(first..last), 0..max)
        .wrap_err("Failed to build playcounts chart")?;

    chart
        .configure_mesh()
        .light_line_style(BLACK.mix(0.0))
        .disable_x_mesh()
        .x_labels(10)
        .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month() as u8))
        .y_desc("Monthly playcount")
        .label_style(("sans-serif", 20_i32, &WHITE))
        .bold_line_style(WHITE.mix(0.3))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
        .draw()
        .wrap_err("Failed to draw playcounts mesh")?;

    draw_area(
        &mut chart,
        PLAYCOUNTS_AREA_COLOR,
        0.5,
        PLAYCOUNTS_BORDER_COLOR,
        0.6,
        playcounts,
        "Monthly playcount",
    )
    .wrap_err("Failed to draw playcount area")
}

const REPLAYS_AREA_COLOR: RGBColor = RGBColor(0, 246, 193);
const REPLAYS_BORDER_COLOR: RGBColor = RGBColor(40, 246, 205);

fn draw_replays(replays: &[MonthlyCount], canvas: &Area<'_>) -> Result<()> {
    let (first, last, max) = first_last_max(replays);
    let label_area = replay_label_area(max);

    let mut chart = ChartBuilder::on(canvas)
        .margin(9_i32)
        .x_label_area_size(20_i32)
        .y_label_area_size(label_area)
        .build_cartesian_2d(Monthly(first..last), 0..max)
        .wrap_err("Failed to build replay chart")?;

    chart
        .configure_mesh()
        .light_line_style(BLACK.mix(0.0))
        .disable_x_mesh()
        .x_labels(10)
        .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month() as u8))
        .y_desc("Replays watched")
        .label_style(("sans-serif", 20_i32, &WHITE))
        .bold_line_style(WHITE.mix(0.3))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
        .draw()
        .wrap_err("Failed to draw replay mesh")?;

    draw_area(
        &mut chart,
        REPLAYS_AREA_COLOR,
        0.2,
        REPLAYS_BORDER_COLOR,
        1.0,
        replays,
        "Replays watched",
    )
    .wrap_err("Failed to draw replay area")
}

fn draw_both(
    playcounts: &[MonthlyCount],
    replays: &[MonthlyCount],
    canvas: &Area<'_>,
) -> Result<()> {
    let (left_first, left_last, left_max) = first_last_max(playcounts);
    let (right_first, right_last, right_max) = first_last_max(replays);
    let right_label_area = replay_label_area(right_max);

    let x_min = left_first.min(right_first);
    let x_max = left_last.max(right_last);

    let mut chart = ChartBuilder::on(canvas)
        .margin(9_i32)
        .x_label_area_size(20_i32)
        .y_label_area_size(75_i32)
        .right_y_label_area_size(right_label_area)
        .build_cartesian_2d(Monthly(x_min..x_max), 0..left_max)
        .wrap_err("Failed to build dual chart")?
        .set_secondary_coord(Monthly(x_min..x_max), 0..right_max);

    // Mesh and labels
    chart
        .configure_mesh()
        .light_line_style(BLACK.mix(0.0))
        .disable_x_mesh()
        .x_labels(10)
        .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month() as u8))
        .y_desc("Monthly playcount")
        .label_style(("sans-serif", 20_i32, &WHITE))
        .bold_line_style(WHITE.mix(0.3))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
        .draw()
        .wrap_err("Failed to draw primary mesh")?;

    chart
        .configure_secondary_axes()
        .y_desc("Replays watched")
        .label_style(("sans-serif", 20_i32, &WHITE))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
        .draw()
        .wrap_err("Failed to draw secondary mesh")?;

    draw_area(
        &mut chart,
        PLAYCOUNTS_AREA_COLOR,
        0.5,
        PLAYCOUNTS_BORDER_COLOR,
        0.6,
        playcounts,
        "Monthly playcount",
    )
    .wrap_err("Failed to draw playcounts area")?;

    // Draw replay watched area
    // Can't use `draw_area` since it's for the secondary y-axis
    let iter = replays
        .iter()
        .map(|MonthlyCount { start_date, count }| (*start_date, *count));

    let area_color = REPLAYS_AREA_COLOR;
    let border_color = REPLAYS_BORDER_COLOR;
    let series = AreaSeries::new(iter, 0, area_color.mix(0.2).filled());

    chart
        .draw_secondary_series(series.border_style(border_color.stroke_width(1)))
        .wrap_err("Failed to draw replays area")?
        .label("Replays watched")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 20, y)], border_color.stroke_width(2))
        });

    // Draw circles
    let circles = replays.iter().map(|MonthlyCount { start_date, count }| {
        let style = border_color.stroke_width(1).filled();

        Circle::new((*start_date, *count), 2_i32, style)
    });

    chart
        .draw_secondary_series(circles)
        .wrap_err("Failed to draw replays circles")?;

    // Legend
    chart
        .configure_series_labels()
        .background_style(RGBColor(7, 23, 17))
        .position(SeriesLabelPosition::UpperLeft)
        .legend_area_size(45_i32)
        .label_font(("sans-serif", 20_i32, &WHITE))
        .draw()
        .wrap_err("Failed to draw legend")?;

    Ok(())
}

fn draw_area(
    chart: &mut Chart<'_, '_>,
    area_color: RGBColor,
    area_mix: f64,
    border_color: RGBColor,
    border_mix: f64,
    monthly_counts: &[MonthlyCount],
    label: &str,
) -> Result<()> {
    // Draw area
    let iter = monthly_counts
        .iter()
        .map(|MonthlyCount { start_date, count }| (*start_date, *count));

    let series = AreaSeries::new(iter, 0, area_color.mix(area_mix).filled());

    chart
        .draw_series(series.border_style(border_color.stroke_width(1)))
        .wrap_err("Failed to draw area")?
        .label(label)
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 20, y)], area_color.stroke_width(2))
        });

    // Draw circles
    let circles = monthly_counts
        .iter()
        .map(move |MonthlyCount { start_date, count }| {
            let style = border_color.mix(border_mix).filled();

            Circle::new((*start_date, *count), 2_i32, style)
        });

    chart
        .draw_series(circles)
        .wrap_err("Failed to draw circles")?;

    Ok(())
}

fn replay_label_area(max: i32) -> i32 {
    match max {
        n if n < 10 => 40,
        n if n < 100 => 50,
        n if n < 1000 => 60,
        n if n < 10_000 => 70,
        n if n < 100_000 => 80,
        _ => 90,
    }
}

fn first_last_max(counts: &[MonthlyCount]) -> (Date, Date, i32) {
    let first = counts.first().unwrap().start_date;
    let last = counts.last().unwrap().start_date;
    let max = counts.iter().map(|c| c.count).max();

    (first, last, max.map_or(2, |m| m.max(2)))
}

fn prepare_monthly_counts(
    user: &mut CachedUser,
    flags: ProfileGraphFlags,
) -> (Vec<MonthlyCount>, Vec<MonthlyCount>) {
    let mut playcounts = rkyv::api::deserialize_using::<_, _, Panic>(
        With::<_, Map<MonthlyCountRkyv>>::cast(&user.monthly_playcounts),
        &mut (),
    )
    .always_ok();

    let mut replays = rkyv::api::deserialize_using::<_, _, Panic>(
        With::<_, Map<MonthlyCountRkyv>>::cast(&user.replays_watched_counts),
        &mut (),
    )
    .always_ok();

    // Spoof missing months
    if flags.playcount() {
        spoof_monthly_counts(&mut playcounts);
    }

    // Spoof missing replays
    if !flags.replays() {
        // nothing to do
    } else if !flags.playcount() {
        let now = OffsetDateTime::now_utc();
        let year = now.year();
        let month = now.month();
        let start_date = Date::from_calendar_date(year, month, 1).unwrap();

        if replays.last().map(|c| c.start_date < start_date) == Some(true) {
            let count = MonthlyCount {
                start_date,
                count: 0,
            };

            replays.push(count);
        }

        spoof_monthly_counts(&mut replays);
    } else {
        // For every month in playcounts, make sure there is one in replays
        for (i, start_date) in playcounts.iter().map(|c| c.start_date).enumerate() {
            let cond = replays.get(i).map(|c| c.start_date == start_date);

            if cond != Some(true) {
                let count = MonthlyCount {
                    start_date,
                    count: 0,
                };

                replays.insert(i, count);
            }
        }
    }

    (playcounts, replays)
}

fn spoof_monthly_counts(counts: &mut Vec<MonthlyCount>) {
    // Fill in months inbetween entries
    let (mut year, mut month) = match counts.as_slice() {
        [] | [_] => return,
        [first, ..] => (first.start_date.year(), first.start_date.month()),
    };

    let mut i = 1;

    while i < counts.len() {
        month = month.next();
        year += (month == Month::January) as i32;
        let date = Date::from_calendar_date(year, month, 1).unwrap();

        if date < counts[i].start_date {
            let count = MonthlyCount {
                start_date: date,
                count: 0,
            };

            counts.insert(i, count);
        }

        i += 1;
    }

    // Pad months at the end
    let now = OffsetDateTime::now_utc().date();
    let mut next = counts[counts.len() - 1].start_date;

    next = next.replace_month(next.month().next()).unwrap();

    if next.month() == Month::January {
        next = next.replace_year(next.year() + 1).unwrap();
    }

    while next < now {
        let count = MonthlyCount {
            start_date: next,
            count: 0,
        };

        counts.push(count);

        next = next.replace_month(next.month().next()).unwrap();

        if next.month() == Month::January {
            next = next.replace_year(next.year() + 1).unwrap();
        }
    }
}

enum GraphResult {
    Ok(Vec<u8>),
    NotEnoughDatapoints,
    NoBadges,
}

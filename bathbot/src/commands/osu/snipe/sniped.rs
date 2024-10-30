use std::ops;

use bathbot_macros::command;
use bathbot_model::SnipedWeek;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    datetime::DATE_FORMAT,
    matcher, MessageBuilder,
};
use eyre::{ContextCompat, Report, Result, WrapErr};
use plotters::{
    coord::{
        ranged1d::{DefaultFormatting, KeyPointHint, SegmentedCoord},
        types::RangedCoordu32,
        Shift,
    },
    prelude::*,
};
use plotters_skia::SkiaBackend;
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use skia_safe::{surfaces, EncodedImageFormat};
use time::Date;
use twilight_model::guild::Permissions;

use super::{SnipeGameMode, SnipePlayerSniped};
use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, SnipedEmbed},
    manager::redis::osu::UserArgs,
    Context,
};

#[command]
#[desc("Sniped users of the last 8 weeks")]
#[help(
    "Sniped users of the last 8 weeks.\n\
    Data for osu!standard originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("snipes")]
#[group(Osu)]
async fn prefix_sniped(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipePlayerSniped::args(args, None);

    player_sniped(CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Sniped ctb users of the last 8 weeks")]
#[help(
    "Sniped ctb users of the last 8 weeks.\n\
    Data for osu!catch originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("snipedc", "snipedcatch", "snipesctb", "snipescatch")]
#[group(Catch)]
async fn prefix_snipedctb(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipePlayerSniped::args(args, Some(GameMode::Catch));

    player_sniped(CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Sniped mania users of the last 8 weeks")]
#[help(
    "Sniped mania users of the last 8 weeks.\n\
    Data for osu!mania originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("snipedm", "snipesmania")]
#[group(Mania)]
async fn prefix_snipedmania(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipePlayerSniped::args(args, Some(GameMode::Mania));

    player_sniped(CommandOrigin::from_msg(msg, permissions), args).await
}

pub(super) async fn player_sniped(
    orig: CommandOrigin<'_>,
    args: SnipePlayerSniped<'_>,
) -> Result<()> {
    let (user_id, mode) = user_id_mode!(orig, args);
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let client = Context::client();

    let (user_id, username, country_code) = match &user {
        RedisData::Original(user) => (
            user.user_id,
            user.username.as_str(),
            user.country_code.as_str(),
        ),
        RedisData::Archive(user) => (
            user.user_id,
            user.username.as_str(),
            user.country_code.as_str(),
        ),
    };

    let (mut sniper, mut snipee) = if Context::huismetbenen()
        .is_supported(country_code, mode)
        .await
    {
        let sniper_fut = client.get_sniped_players(user_id, true, mode);
        let snipee_fut = client.get_sniped_players(user_id, false, mode);

        match tokio::try_join!(sniper_fut, snipee_fut) {
            Ok(tuple) => tuple,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to get sniper or snipee"));
            }
        }
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        return orig.error(content).await;
    };

    let graph = match graphs(username, &mut sniper, &mut snipee, W, H) {
        Ok(graph_option) => graph_option,
        Err(err) => {
            warn!(?err, "Failed to create graph");

            None
        }
    };

    let embed = SnipedEmbed::new(&user, sniper, snipee).build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.attachment("sniped_graph.png", bytes);
    }

    orig.create_message(builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

fn accumulate_counts(weeks: &mut [SnipedWeek]) {
    for week in weeks {
        for i in (1..week.players.len()).rev() {
            week.players[i - 1].count += week.players[i].count;
        }
    }
}

pub fn graphs(
    name: &str,
    sniper: &mut [SnipedWeek],
    snipee: &mut [SnipedWeek],
    w: u32,
    h: u32,
) -> Result<Option<Vec<u8>>> {
    if sniper.is_empty() && snipee.is_empty() {
        return Ok(None);
    }

    accumulate_counts(sniper);
    accumulate_counts(snipee);

    let mut surface =
        surfaces::raster_n32_premul((w as i32, h as i32)).wrap_err("Failed to create surface")?;

    {
        let root = SkiaBackend::new(surface.canvas(), w, h).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        match (sniper.is_empty(), snipee.is_empty()) {
            (false, true) => draw_sniper(&root, name, sniper).wrap_err("failed to draw sniper")?,
            (true, false) => draw_snipee(&root, name, snipee).wrap_err("failed to draw snipee")?,
            (false, false) => {
                let (left, right) = root.split_horizontally(w / 2);
                draw_sniper(&left, name, sniper).wrap_err("failed to draw sniper")?;
                draw_snipee(&right, name, snipee).wrap_err("failed to draw snipee")?
            }
            (true, true) => unreachable!(),
        }
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(Some(png_bytes))
}

type ContextType<'a> = Cartesian2d<SegmentedCoord<SnipedWeeksCoord<'a>>, RangedCoordu32>;

fn draw_sniper<DB: DrawingBackend>(
    root: &DrawingArea<DB, Shift>,
    name: &str,
    sniper: &[SnipedWeek],
) -> Result<()> {
    let max = sniper[0].players[0].count;

    let mut chart = ChartBuilder::on(root)
        .x_label_area_size(30)
        .y_label_area_size(35)
        .margin_right(5)
        .caption(format!("Sniped by {name}"), ("sans-serif", 25, &WHITE))
        .build_cartesian_2d(SnipedWeeksCoord::new(sniper).into_segmented(), 0..max + 1)
        .map_err(|e| Report::msg(e.to_string()))
        .wrap_err("Failed to build chart")?;

    draw_mesh(&mut chart)?;
    draw_histogram_blocks(sniper, &mut chart).wrap_err("Failed to draw histogram blocks")?;
    draw_legend(&mut chart)?;

    Ok(())
}

fn draw_snipee<DB: DrawingBackend>(
    root: &DrawingArea<DB, Shift>,
    name: &str,
    snipee: &[SnipedWeek],
) -> Result<()> {
    let max = snipee[0].players[0].count;

    let mut chart = ChartBuilder::on(root)
        .x_label_area_size(30)
        .y_label_area_size(35)
        .margin_right(5)
        .caption(format!("Sniped {name}"), ("sans-serif", 25, &WHITE))
        .build_cartesian_2d(SnipedWeeksCoord::new(snipee).into_segmented(), 0..max + 1)
        .map_err(|e| Report::msg(e.to_string()))
        .wrap_err("Failed to build chart")?;

    draw_mesh(&mut chart)?;
    draw_histogram_blocks(snipee, &mut chart).wrap_err("Failed to draw histogram blocks")?;
    draw_legend(&mut chart)?;

    Ok(())
}

fn draw_mesh<DB: DrawingBackend>(chart: &mut ChartContext<'_, DB, ContextType<'_>>) -> Result<()> {
    chart
        .configure_mesh()
        .disable_x_mesh()
        .x_label_formatter(&|date| match date {
            SegmentValue::CenterOf(date) | SegmentValue::Exact(date) => {
                date.format(DATE_FORMAT).unwrap()
            }
            _ => unreachable!(),
        })
        .label_style(("sans-serif", 15, &WHITE))
        .bold_line_style(WHITE.mix(0.3))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
        .draw()
        .map_err(|e| Report::msg(e.to_string()))
        .wrap_err("Failed to draw mesh")
}

fn draw_histogram_blocks<'a, DB: DrawingBackend + 'a>(
    weeks: &'a [SnipedWeek],
    chart: &mut ChartContext<'a, DB, ContextType<'a>>,
) -> Result<()> {
    for (i, player) in weeks[0].players.iter().enumerate() {
        let count_iter = || {
            weeks.iter().rev().filter_map(|week| {
                let player = week
                    .players
                    .iter()
                    .find(|p| p.username == player.username)?;

                Some((week.until.date(), player.count))
            })
        };

        // Draw block
        let color = HSLColor(i as f64 * 0.1, 0.5, 0.5);

        let series = Histogram::vertical(chart)
            .data(count_iter())
            .style(color.mix(0.75).filled());

        chart
            .draw_series(series)
            .map_err(|e| Report::msg(e.to_string()))
            .wrap_err("Failed to draw block")?
            .label(player.username.as_str())
            .legend(move |(x, y)| Circle::new((x, y), 4, color.filled()));

        // Draw border
        let color = HSLColor(i as f64 * 0.1, 0.5, 0.3);

        let series = Histogram::vertical(chart).data(count_iter()).style(color);

        chart
            .draw_series(series)
            .map_err(|e| Report::msg(e.to_string()))
            .wrap_err("Failed to draw border")?;
    }

    Ok(())
}

fn draw_legend<'a, DB: DrawingBackend + 'a>(
    chart: &mut ChartContext<'a, DB, ContextType<'_>>,
) -> Result<()> {
    chart
        .configure_series_labels()
        .border_style(WHITE.mix(0.6).stroke_width(1))
        .background_style(RGBColor(7, 23, 17))
        .position(SeriesLabelPosition::UpperLeft)
        .legend_area_size(13)
        .label_font(("sans-serif", 15, FontStyle::Bold, &WHITE))
        .draw()
        .map_err(|e| Report::msg(e.to_string()))
        .wrap_err("Failed to draw legend")
}

// Custom coordinate system so that dates can be given through ownership
// instead of by reference
#[derive(Copy, Clone)]
struct SnipedWeeksCoord<'a> {
    weeks: &'a [SnipedWeek],
}

impl<'a> SnipedWeeksCoord<'a> {
    fn new(weeks: &'a [SnipedWeek]) -> Self {
        Self { weeks }
    }
}

impl<'a> Ranged for SnipedWeeksCoord<'a> {
    type FormatOption = DefaultFormatting;
    type ValueType = Date;

    fn map(&self, date: &Self::ValueType, (start, end): (i32, i32)) -> i32 {
        match self
            .weeks
            .iter()
            .rev()
            .position(|week| week.until.date() == *date)
        {
            Some(pos) => {
                let pixel_span = end - start;
                let value_span = self.weeks.len() - 1;

                (f64::from(start)
                    + f64::from(pixel_span)
                        * (f64::from(pos as u32) / f64::from(value_span as u32)))
                .round() as i32
            }
            None => start,
        }
    }

    fn key_points<Hint: KeyPointHint>(&self, hint: Hint) -> Vec<Self::ValueType> {
        let max_points = hint.max_num_points();
        let intervals = (self.weeks.len() - 1) as f64;
        let step = (intervals / max_points as f64 + 1.0) as usize;

        self.weeks
            .iter()
            .rev()
            .step_by(step)
            .map(|week| week.until.date())
            .collect()
    }

    fn range(&self) -> ops::Range<Self::ValueType> {
        match self.weeks {
            [last, .., first] => first.until.date()..last.until.date(),
            [single] => {
                let date = single.until.date();

                date..date
            }
            [] => panic!("empty weeks"),
        }
    }
}

impl<'a> DiscreteRanged for SnipedWeeksCoord<'a> {
    fn size(&self) -> usize {
        self.weeks.len()
    }

    fn index_of(&self, date: &Date) -> Option<usize> {
        self.weeks
            .iter()
            .rev()
            .position(|week| &week.until.date() == date)
    }

    fn from_index(&self, idx: usize) -> Option<Date> {
        self.weeks
            .get(self.weeks.len() - (idx + 1))
            .map(|week| week.until.date())
    }
}

impl<'m> SnipePlayerSniped<'m> {
    fn args(mut args: Args<'m>, mode: Option<GameMode>) -> Self {
        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Self {
            mode: mode.and_then(SnipeGameMode::try_from_mode),
            name,
            discord,
        }
    }
}

use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand, command};
use bathbot_model::{
    command_fields::GameModeOption,
    rosu_v2::matchmaking::{
        MatchmakingPoolTypeRkyv, MatchmakingResultRkyv, MatchmakingUserEloHistoryRkyv,
        MatchmakingUserStatsRkyv,
    },
};
use bathbot_util::{MessageBuilder, attachment, constants::GENERAL_ISSUE, matcher};
use eyre::{ContextCompat, Report, Result, WrapErr};
use plotters::{
    prelude::{ChartBuilder, Circle, IntoDrawingArea},
    series::AreaSeries,
    style::{Color, GREEN, RED, RGBColor, ShapeStyle, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rkyv::Deserialize;
use rkyv::rancor::{Panic, ResultExt, Strategy};
use rosu_v2::prelude::OsuError;
use skia_safe::{EncodedImageFormat, surfaces};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{Id, marker::UserMarker};

use super::user_not_found;
use crate::{
    Context,
    active::{
        ActiveMessages,
        impls::{RankedPagination, build_embed},
    },
    commands::{DISCORD_OPTION_DESC, DISCORD_OPTION_HELP},
    core::commands::CommandOrigin,
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(name = "ranked", desc = "Display ranked play statistics of a user")]
pub struct Ranked<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

async fn slash_ranked(mut command: InteractionCommand) -> Result<()> {
    let args = Ranked::from_interaction(command.input_data())?;

    ranked((&mut command).into(), args).await
}

#[command]
#[desc("Display ranked play statistics of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("rankedplay")]
#[group(AllModes)]
async fn prefix_ranked(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => Ranked {
                name: None,
                discord: Some(id),
                mode: None,
            },
            None => Ranked {
                name: Some(Cow::Borrowed(arg)),
                discord: None,
                mode: None,
            },
        },
        None => Ranked {
            name: None,
            discord: None,
            mode: None,
        },
    };

    ranked(msg.into(), args).await
}

async fn ranked(orig: CommandOrigin<'_>, args: Ranked<'_>) -> Result<()> {
    let owner = orig.user_id()?;
    let (user_id, mode) = user_id_mode!(orig, args);

    // Retrieve the user and their matchmaking statistics
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get user"));
        }
    };

    // Keep the stats of the pools of the given mode only
    let mut pools: Vec<MatchmakingUserStatsRkyv> = user
        .matchmaking_stats
        .iter()
        .filter(|stats| {
            stats
                .pool
                .as_ref()
                .filter(|pool| pool.mode == mode)
                .is_some()
        })
        .map(|stats| {
            stats
                .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                .always_ok()
        })
        .collect();

    if pools.is_empty() {
        let content = format!("`{}` has no ranked play statistics", user.username.as_str());

        return orig.error(content).await;
    }

    // Ranked pools first, then the highest rating within each pool type
    pools.sort_unstable_by(|a, b| {
        let ranked_play = |stats: &MatchmakingUserStatsRkyv| -> bool {
            !stats
                .pool
                .as_ref()
                .is_some_and(|pool| matches!(pool.kind, MatchmakingPoolTypeRkyv::RankedPlay))
        };

        ranked_play(a)
            .cmp(&ranked_play(b))
            .then_with(|| b.rating.cmp(&a.rating))
    });

    // A single pool doesn't need pagination, so it is sent as a plain message
    if let [stats] = pools.as_slice() {
        let graph = if stats.recent_history.len() >= 2 {
            Some(render_rating_graph(&stats.recent_history)?)
        } else {
            None
        };

        let builder = match graph {
            Some(graph) => {
                let embed = build_embed(&user, stats).image(attachment("ranked.png"));

                MessageBuilder::new()
                    .embed(embed)
                    .attachment("ranked.png", graph)
            }
            None => MessageBuilder::new().embed(build_embed(&user, stats)),
        };

        orig.create_message(builder).await?;

        return Ok(());
    }

    let pagination = RankedPagination::builder()
        .user(user)
        .pools(pools.into_boxed_slice())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

const W: u32 = 1350;
const H: u32 = 711;
const MAX_POINTS: usize = 100;

fn render_rating_graph(history: &[MatchmakingUserEloHistoryRkyv]) -> Result<Vec<u8>> {
    // Cap the amount of matches to keep the graph readable
    let capped = history.len() > MAX_POINTS;
    let history = &history[..history.len().min(MAX_POINTS)];

    let n = history.len();

    let (min, max) = history
        .iter()
        .fold((i32::MAX, i32::MIN), |(min, max), entry| {
            (min.min(entry.elo_after), max.max(entry.elo_after))
        });

    // Pad the rating range so that markers on the edges don't overflow the plot area
    let spread = max - min;

    let pad = if spread == 0 {
        10
    } else {
        (spread * 8 / 100).max(4)
    };

    let (min_y, max_y) = (min - pad, max + pad);

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

        // A floating point x range maps match numbers continuously, so the first
        // match sits exactly on the y axis and the last one exactly on the right
        // border (an integer range would map discrete values to bin centers instead).
        // Requesting n ticks yields exactly one integer tick per match (1..=n), and
        // the label formatter renders the tick values as integers again.
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(40)
            .y_label_area_size(70)
            .margin(10)
            .margin_left(6)
            .build_cartesian_2d(1.0..n as f64, min_y..max_y)
            .wrap_err("Failed to build chart")?;

        chart
            .configure_mesh()
            .disable_y_mesh()
            .x_labels(n)
            .x_label_formatter(&|value| value.round().to_string())
            .x_desc("Matches")
            .y_desc("Rating")
            .label_style(("sans-serif", 15, &WHITE))
            .bold_line_style(WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("Failed to draw mesh")?;

        // The API returns the history in newest first order, so plot the matches in
        // oldest first order, from left to right
        let points: Vec<(f64, i32)> = history
            .iter()
            .rev()
            .enumerate()
            .map(|(i, entry)| ((i + 1) as f64, entry.elo_after))
            .collect();

        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = style(RGBColor(0, 208, 138)).stroke_width(3);
        let series =
            AreaSeries::new(points.iter().copied(), min_y, area_style).border_style(border_style);
        chart.draw_series(series).wrap_err("Failed to draw area")?;

        // Markers colored by match result
        let radius = if capped { 4_u32 } else { 6_u32 };

        let circles = points
            .iter()
            .zip(history.iter().rev())
            .map(|(&(x, y), entry)| {
                let color = match entry.result {
                    MatchmakingResultRkyv::Win => GREEN,
                    MatchmakingResultRkyv::Loss => RED,
                    MatchmakingResultRkyv::Draw => RGBColor(128, 128, 128),
                };

                let style = ShapeStyle {
                    color: color.to_rgba(),
                    filled: true,
                    stroke_width: 0,
                };

                Circle::new((x, y), radius, style)
            });

        chart
            .draw_series(circles)
            .wrap_err("Failed to draw match markers")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

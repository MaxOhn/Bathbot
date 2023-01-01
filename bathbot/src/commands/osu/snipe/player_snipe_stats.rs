use std::{collections::BTreeMap, sync::Arc};

use bathbot_macros::command;
use eyre::{Report, Result, WrapErr};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::prelude::*;
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};
use time::Date;

use crate::{
    commands::osu::require_link,
    core::commands::CommandOrigin,
    embeds::{EmbedData, PlayerSnipeStatsEmbed},
    manager::redis::{osu::UserArgs, RedisData},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        matcher, Monthly,
    },
    Context,
};

use super::SnipePlayerStats;

#[command]
#[desc("Stats about a user's #1 scores in their country leaderboards")]
#[help(
    "Stats about a user's #1 scores in their country leaderboards.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("pss")]
#[group(Osu)]
async fn prefix_playersnipestats(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> Result<()> {
    let args = match args.next() {
        Some(arg) => match matcher::get_mention_user(arg) {
            Some(id) => SnipePlayerStats {
                name: None,
                discord: Some(id),
            },
            None => SnipePlayerStats {
                name: Some(arg.into()),
                discord: None,
            },
        },
        None => SnipePlayerStats::default(),
    };

    player_stats(ctx, msg.into(), args).await
}

pub(super) async fn player_stats(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: SnipePlayerStats<'_>,
) -> Result<()> {
    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let (country_code, username, user_id) = match &user {
        RedisData::Original(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
        RedisData::Archived(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
    };

    let player_fut = if ctx.huismetbenen().is_supported(country_code).await {
        ctx.client().get_snipe_player(country_code, user_id)
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        return orig.error(&ctx, content).await;
    };

    let player = match player_fut.await {
        Ok(Some(player)) => player,
        Ok(None) => {
            let content = format!("`{username}` has never had any national #1s");
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(&ctx, &builder).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = orig.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(err);
        }
    };

    let graph = match graphs(&player.count_first_history, &player.count_sr_spread, W, H) {
        Ok(graph) => Some(graph),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to create graph"));

            None
        }
    };

    let score_fut = ctx
        .osu()
        .beatmap_user_score(player.oldest_first.map_id, player.user_id)
        .mode(GameMode::Osu);

    let map_fut = ctx.osu_map().map(player.oldest_first.map_id, None);

    let (oldest_score, oldest_map) = match tokio::join!(score_fut, map_fut) {
        (Ok(score), Ok(map)) => (score.score, map),
        (Err(err), _) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get oldest score"));
        }
        (_, Err(err)) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("failed to get map of oldest score"));
        }
    };

    let embed = PlayerSnipeStatsEmbed::new(&user, player, &oldest_score, &oldest_map, &ctx)
        .await
        .build();

    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.attachment("stats_graph.png", bytes);
    }

    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

pub fn graphs(
    history: &BTreeMap<Date, u32>,
    stars: &BTreeMap<i8, Option<u32>>,
    w: u32,
    h: u32,
) -> Result<Vec<u8>> {
    let len = (w * h * 3) as usize; // PIXEL_SIZE = 3
    let mut buf = vec![0; len];

    let style: fn(RGBColor) -> ShapeStyle = |color| ShapeStyle {
        color: color.to_rgba(),
        filled: false,
        stroke_width: 1,
    };

    {
        let root = BitMapBackend::with_buffer(&mut buf, (w, h)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let star_canvas = if history.len() > 1 {
            let (left, right) = root.split_horizontally(3 * w / 5);

            let (min, max) = history
                .iter()
                .map(|(_, n)| *n)
                .fold((u32::MAX, 0), |(min, max), curr| {
                    (min.min(curr), max.max(curr))
                });

            let min = match min < 20 {
                true => 0,
                false => min - min / 11,
            };

            let first = *history.keys().next().unwrap();
            let last = *history.keys().last().unwrap();

            let mut chart = ChartBuilder::on(&left)
                .margin(9)
                .caption("National #1 Count History", ("sans-serif", 30, &WHITE))
                .x_label_area_size(20)
                .y_label_area_size(40)
                .build_cartesian_2d(Monthly(first..last), min..max + 1)
                .wrap_err("failed to build left chart")?;

            // Mesh and labels
            chart
                .configure_mesh()
                .disable_x_mesh()
                .x_labels(8)
                .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month() as u8))
                .label_style(("sans-serif", 15, &WHITE))
                .bold_line_style(WHITE.mix(0.3))
                .axis_style(RGBColor(7, 18, 14))
                .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
                .draw()
                .wrap_err("failed to draw left mesh")?;

            // Draw area
            let iter = history.iter().map(|(date, n)| (*date, *n));
            let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
            let border_style = style(RGBColor(0, 208, 138)).stroke_width(3);
            let series = AreaSeries::new(iter, 0, area_style).border_style(border_style);
            chart
                .draw_series(series)
                .wrap_err("failed to draw left series")?;

            right
        } else {
            root
        };

        // Star spread graph
        let max = stars
            .iter()
            .filter(|(sr, _)| **sr >= 0)
            .map(|(_, n)| n.unwrap_or(0))
            .fold(0, |max, curr| max.max(curr));

        let first = *stars.keys().find(|sr| **sr >= 0).unwrap() as u32;
        let last = *stars.keys().filter(|sr| **sr >= 0).last().unwrap() as u32;

        let mut chart = ChartBuilder::on(&star_canvas)
            .x_label_area_size(30)
            .y_label_area_size(40)
            .margin_right(15)
            .caption("Star rating spread", ("sans-serif", 30, &WHITE))
            .build_cartesian_2d((first..last).into_segmented(), 0..max + 1)
            .wrap_err("failed to build right chart")?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_labels(15)
            .label_style(("sans-serif", 15, &WHITE))
            .bold_line_style(WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw right mesh")?;

        // Histogram bars
        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();

        let iter = stars
            .iter()
            .filter(|(sr, _)| **sr >= 0)
            .map(|(stars, n)| (*stars as u32, n.unwrap_or(0)));

        let series = Histogram::vertical(&chart)
            .style(area_style)
            .data(iter)
            .margin(3);

        chart
            .draw_series(series)
            .wrap_err("failed to draw right series")?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);

    png_encoder
        .write_image(&buf, w, h, ColorType::Rgb8)
        .wrap_err("failed to encode image")?;

    Ok(png_bytes)
}

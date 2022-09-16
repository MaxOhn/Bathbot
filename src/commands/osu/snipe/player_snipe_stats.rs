use std::{collections::BTreeMap, sync::Arc};

use command_macros::command;
use eyre::{Report, Result, WrapErr};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::prelude::*;
use rosu_v2::prelude::{GameMode, OsuError};
use time::Date;

use crate::{
    commands::osu::{get_user, require_link, UserArgs},
    core::commands::CommandOrigin,
    embeds::{EmbedData, PlayerSnipeStatsEmbed},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
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
    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(orig.user_id()?).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get username"));
            }
        },
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::Osu);

    let mut user = match get_user(&ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    // Overwrite default mode
    user.mode = GameMode::Osu;

    let player_fut = if ctx.contains_country(user.country_code.as_str()) {
        ctx.client()
            .get_snipe_player(&user.country_code, user.user_id)
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return orig.error(&ctx, content).await;
    };

    let player = match player_fut.await {
        Ok(counts) => counts,
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to get snipe player"));
            let content = format!("`{name}` has never had any national #1s");
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(&ctx, &builder).await?;

            return Ok(());
        }
    };

    // TODO: dont do this async
    let graph_fut = async { graphs(&player.count_first_history, &player.count_sr_spread, W, H) };

    let oldest_fut = async {
        let valid_oldest = player
            .oldest_first
            .as_ref()
            .filter(|map| map.date.is_some());

        if let Some(oldest) = valid_oldest {
            let score_fut = ctx
                .osu()
                .beatmap_user_score(oldest.beatmap_id, player.user_id)
                .mode(GameMode::Osu);

            match score_fut.await {
                Ok(mut score) => match super::prepare_score(&ctx, &mut score.score).await {
                    Ok(_) => Ok(Some(score.score)),
                    Err(err) => Err(err),
                },
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get oldest data");
                    warn!("{report:?}");

                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    };

    let (graph_result, first_score_result) = tokio::join!(graph_fut, oldest_fut);

    let graph = match graph_result {
        Ok(graph) => Some(graph),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to create graph"));

            None
        }
    };

    let first_score = match first_score_result {
        Ok(score) => score,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user score");

            return Err(report);
        }
    };

    let embed = PlayerSnipeStatsEmbed::new(user, player, first_score, &ctx)
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
    stars: &BTreeMap<u8, u32>,
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
                .bold_line_style(&WHITE.mix(0.3))
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
            .map(|(_, n)| *n)
            .fold(0, |max, curr| max.max(curr));

        let first = *stars.keys().next().unwrap() as u32;
        let last = *stars.keys().last().unwrap() as u32;

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
            .bold_line_style(&WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw right mesh")?;

        // Histogram bars
        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let iter = stars.iter().map(|(stars, n)| (*stars as u32, *n));

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

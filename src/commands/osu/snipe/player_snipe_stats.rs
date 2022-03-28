use std::{collections::BTreeMap, sync::Arc};

use chrono::{Date, Datelike, Utc};
use eyre::Report;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::prelude::*;
use rosu_v2::prelude::{GameMode, OsuError, Username};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user, UserArgs},
    },
    database::OsuData,
    embeds::{EmbedData, PlayerSnipeStatsEmbed},
    error::GraphError,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

#[command]
#[short_desc("Stats about a user's #1 scores in their country leaderboards")]
#[long_desc(
    "Stats about a user's #1 scores in their country leaderboards.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("pss")]
#[bucket("snipe")]
async fn playersnipestats(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match check_user_mention(&ctx, arg).await {
                    Ok(Ok(osu)) => Some(osu.into_username()),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.psql().get_user_osu(msg.author.id).await {
                    Ok(osu) => osu.map(OsuData::into_username),
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
            };

            _playersnipestats(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => super::slash_snipe(ctx, *command).await,
    }
}

pub(super) async fn _playersnipestats(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Username>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::STD);

    let mut user = match get_user(&ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = GameMode::STD;

    let player_fut = if ctx.contains_country(user.country_code.as_str()) {
        ctx.clients
            .custom
            .get_snipe_player(&user.country_code, user.user_id)
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return data.error(&ctx, content).await;
    };

    let player = match player_fut.await {
        Ok(counts) => counts,
        Err(why) => {
            let report = Report::new(why).wrap_err("failed to retrieve snipe player");
            warn!("{report:?}");
            let content = format!("`{name}` has never had any national #1s");
            let builder = MessageBuilder::new().embed(content);
            data.create_message(&ctx, builder).await?;

            return Ok(());
        }
    };

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
                .mode(GameMode::STD);

            match score_fut.await {
                Ok(mut score) => match super::prepare_score(&ctx, &mut score.score).await {
                    Ok(_) => Ok(Some(score.score)),
                    Err(why) => Err(why),
                },
                Err(why) => {
                    let report = Report::new(why).wrap_err("faield to retrieve oldest data");
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
            warn!("{:?}", Report::new(err));

            None
        }
    };

    let first_score = match first_score_result {
        Ok(score) => score,
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let embed_data = PlayerSnipeStatsEmbed::new(user, player, first_score, &ctx).await;

    // Sending the embed
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.file("stats_graph.png", bytes);
    }

    data.create_message(&ctx, builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

pub fn graphs(
    history: &BTreeMap<Date<Utc>, u32>,
    stars: &BTreeMap<u8, u32>,
    w: u32,
    h: u32,
) -> Result<Vec<u8>, GraphError> {
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
        root.fill(&background)?;

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
                .build_cartesian_2d((first..last).monthly(), min..max + 1)?;

            // Mesh and labels
            chart
                .configure_mesh()
                .disable_x_mesh()
                .x_labels(8)
                .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month()))
                .label_style(("sans-serif", 15, &WHITE))
                .bold_line_style(&WHITE.mix(0.3))
                .axis_style(RGBColor(7, 18, 14))
                .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
                .draw()?;

            // Draw area
            let iter = history.iter().map(|(date, n)| (*date, *n));
            let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
            let border_style = style(RGBColor(0, 208, 138)).stroke_width(3);
            let series = AreaSeries::new(iter, 0, area_style).border_style(border_style);
            chart.draw_series(series)?;

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
            .build_cartesian_2d((first..last).into_segmented(), 0..max + 1)?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_labels(15)
            .label_style(("sans-serif", 15, &WHITE))
            .bold_line_style(&WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
            .draw()?;

        // Histogram bars
        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let iter = stars.iter().map(|(stars, n)| (*stars as u32, *n));

        let series = Histogram::vertical(&chart)
            .style(area_style)
            .data(iter)
            .margin(3);

        chart.draw_series(series)?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, w, h, ColorType::Rgb8)?;

    Ok(png_bytes)
}

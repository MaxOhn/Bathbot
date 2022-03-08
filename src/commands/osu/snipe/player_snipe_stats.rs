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

    let graph_fut = async { graphs(&player.count_first_history, &player.count_sr_spread) };

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
        Ok(graph_option) => graph_option,
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

    if let Some(bytes) = graph.as_deref() {
        builder = builder.file("stats_graph.png", bytes);
    }

    data.create_message(&ctx, builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

fn graphs(
    history: &BTreeMap<Date<Utc>, u32>,
    stars: &BTreeMap<u8, u32>,
) -> Result<Option<Vec<u8>>, GraphError> {
    static LEN: usize = W as usize * H as usize;
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;

        let star_canvas = if history.len() > 1 {
            let (left, right) = root.split_horizontally(3 * W / 5);

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
                .caption("National #1 Count History", ("sans-serif", 30))
                .x_label_area_size(20)
                .y_label_area_size(40)
                .build_cartesian_2d((first..last).monthly(), min..max)?;

            // Mesh and labels
            chart
                .configure_mesh()
                .disable_x_mesh()
                .x_labels(10)
                .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month()))
                .draw()?;

            // Draw area
            chart.draw_series(
                AreaSeries::new(
                    history.iter().map(|(date, n)| (*date, *n)),
                    min,
                    &BLUE.mix(0.2),
                )
                .border_style(&BLUE),
            )?;

            // Draw circles
            chart.draw_series(
                history
                    .iter()
                    .map(|(y, m)| Circle::new((*y, *m), 2, BLUE.filled())),
            )?;

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
            .caption("Star rating spread", ("sans-serif", 30))
            .build_cartesian_2d((first..last).into_segmented(), 0..max)?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_labels(15)
            .draw()?;

        // Histogram bars
        chart.draw_series(
            Histogram::vertical(&chart)
                .style(RED.mix(0.5).filled())
                .data(stars.iter().map(|(stars, n)| (*stars as u32, *n))),
        )?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, W, H, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}

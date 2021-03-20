use super::prepare_score;
use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, PlayerSnipeStatsEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt, SNIPE_COUNTRIES},
    BotResult, Context,
};

use chrono::{Date, Datelike, Utc};
use image::{png::PngEncoder, ColorType};
use plotters::prelude::*;
use rosu_v2::model::GameMode;
use std::{collections::BTreeMap, sync::Arc};
use twilight_model::channel::Message;

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
async fn playersnipestats(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let user = match ctx.osu().user(name.as_str()).mode(GameMode::STD).await {
        Ok(user) => user,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let player_fut = if SNIPE_COUNTRIES.contains_key(user.country_code.as_str()) {
        ctx.clients
            .custom
            .get_snipe_player(&user.country_code, user.user_id)
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return msg.error(&ctx, content).await;
    };

    let player = match player_fut.await {
        Ok(counts) => counts,
        Err(why) => {
            unwind_error!(warn, why, "Error for command `playersnipestats`: {}");
            let content = format!("`{}` has never had any national #1s", name);

            return msg.send_response(&ctx, content).await;
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
                Ok(mut score) => match prepare_score(&ctx, &mut score.score).await {
                    Ok(_) => Ok(Some(score.score)),
                    Err(why) => Err(why),
                },
                Err(why) => {
                    unwind_error!(warn, why, "Error while retrieving oldest data: {}");

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
        Err(why) => {
            unwind_error!(warn, why, "Error while creating snipe player graph: {}");

            None
        }
    };

    let first_score = match first_score_result {
        Ok(score) => score,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let data = PlayerSnipeStatsEmbed::new(user, player, first_score).await;

    // Sending the embed
    let embed = data.build_owned().build()?;
    let m = ctx.http.create_message(msg.channel_id).embed(embed)?;

    let response = if let Some(graph) = graph {
        m.attachment("stats_graph.png", graph).await?
    } else {
        m.await?
    };

    response.reaction_delete(&ctx, msg.author.id);

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

fn graphs(
    history: &BTreeMap<Date<Utc>, u32>,
    stars: &BTreeMap<u8, u32>,
) -> BotResult<Option<Vec<u8>>> {
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
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}

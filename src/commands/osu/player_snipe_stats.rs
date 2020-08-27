use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, PlayerSnipeStatsEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt, SNIPE_COUNTRIES},
    BotResult, Context,
};

use chrono::{Date, Datelike, Utc};
use image::{png::PNGEncoder, ColorType};
use plotters::{coord::IntoMonthly, prelude::*};
use rosu::{
    backend::{BeatmapRequest, ScoreRequest},
    models::GameMode,
};
use std::{collections::BTreeMap, sync::Arc};
use twilight::model::channel::Message;

#[command]
#[short_desc("Various stats about a user's scores in their country leaderbords")]
#[long_desc(
    "Various stats about a user's scores in their country leaderboards.\n\
    All data originates from Mr Helix's website [huismetbenen](https://snipe.huismetbenen.nl/)."
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
    let user = match ctx.osu_user(&name, GameMode::STD).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("Could not find user `{}`", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    if !SNIPE_COUNTRIES.contains_key(&user.country) {
        let content = format!("The country {} is not supported :(", user.country);
        return msg.error(&ctx, content).await;
    }
    let req = ctx.clients.custom.get_snipe_player(&user);
    let player = match req.await {
        Ok(counts) => counts,
        Err(why) => {
            let content = format!("`{}` has never had any national #1s", name);
            let _ = msg.respond(&ctx, content).await;
            return Err(why);
        }
    };
    let graph = match graphs(&player.count_first_history, &player.count_sr_spread) {
        Ok(graph_option) => graph_option,
        Err(why) => {
            warn!("Error while creating snipe player graph: {}", why);
            None
        }
    };
    let first_score = if let Some(ref oldest) = player.oldest_first {
        let map_id = oldest.beatmap_id;
        let score_req = ScoreRequest::with_map_id(map_id)
            .user_id(player.user_id)
            .mode(GameMode::STD)
            .queue_single(ctx.osu());
        let map_req = BeatmapRequest::new()
            .map_id(map_id)
            .mode(GameMode::STD)
            .queue_single(ctx.osu());
        match tokio::try_join!(score_req, map_req) {
            Ok((Some(score), Some(map))) => Some((score, map)),
            Ok((None, _)) => {
                warn!("No api result for score");
                None
            }
            Ok((_, None)) => {
                warn!("No api result for beatmap");
                None
            }
            Err(why) => {
                warn!("Error while retrieving oldest data: {}", why);
                None
            }
        }
    } else {
        None
    };
    let data = PlayerSnipeStatsEmbed::new(user, player, first_score).await;

    // Sending the embed
    let embed = data.build().build()?;
    let m = ctx.http.create_message(msg.channel_id).embed(embed)?;
    if let Some(graph) = graph {
        m.attachment("stats_graph.png", graph).await?
    } else {
        m.await?
    };
    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

fn graphs(
    history: &BTreeMap<Date<Utc>, u32>,
    stars: &BTreeMap<u8, u32>,
) -> BotResult<Option<Vec<u8>>> {
    if history.is_empty() {
        return Ok(None);
    }
    static LEN: usize = W as usize * H as usize;
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
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3
    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;
        let (left, right) = root.split_horizontally(3 * W / 5);
        let mut chart = ChartBuilder::on(&left)
            .margin(9)
            .caption("National #1 Count History", ("sans-serif", 30))
            .x_label_area_size(20)
            .y_label_area_size(40)
            .build_ranged((first..last).monthly(), min..max)?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
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

        // Star spread graph
        let max = stars
            .iter()
            .map(|(_, n)| *n)
            .fold(0, |max, curr| max.max(curr));
        let first = *stars.keys().next().unwrap() as u32;
        let last = *stars.keys().last().unwrap() as u32;

        let mut chart = ChartBuilder::on(&right)
            .x_label_area_size(30)
            .y_label_area_size(40)
            .margin_right(15)
            .caption("Star rating spread", ("sans-serif", 30))
            .build_ranged(first..last, 0..max)?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .line_style_1(&WHITE.mix(0.3))
            .x_label_offset(30)
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
    let png_encoder = PNGEncoder::new(&mut png_bytes);
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;
    Ok(Some(png_bytes))
}

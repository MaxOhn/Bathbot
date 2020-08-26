use crate::{
    arguments::{Args, NameArgs},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use chrono::{Date, Datelike, Utc};
use image::{png::PNGEncoder, ColorType};
use plotters::{coord::IntoMonthly, prelude::*};
use rosu::models::GameMode;
use std::{collections::BTreeMap, sync::Arc};
use twilight::model::channel::Message;

#[command]
#[short_desc("Various stats about a user's scores in their country leaderbords")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("ns")]
async fn nationalstats(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
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
    // let counts = match super::get_globals_count(&ctx, &user.username, mode).await {
    //     Ok(counts) => counts,
    //     Err(why) => {
    //         let content = "Some issue with the osustats website, blame bade";
    //         let _ = msg.error(&ctx, content).await;
    //         return Err(why);
    //     }
    // };
    Ok(())
}

const W: u32 = 1250;
const H: u32 = 350;

fn graphs(history: &BTreeMap<Date<Utc>, u32>, stars: &BTreeMap<u8, u32>) -> BotResult<Vec<u8>> {
    static LEN: usize = W as usize * H as usize;
    let (mut min, max) = history
        .iter()
        .map(|(_, n)| *n)
        .fold((u32::MAX, 0), |(min, max), curr| {
            (min.min(curr), max.max(curr))
        });
    min -= min / 10;
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
            .x_desc("Stars")
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
    Ok(png_bytes)
}

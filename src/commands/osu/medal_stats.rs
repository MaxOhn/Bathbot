use super::request_user;
use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, MedalStatsEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context, Error,
};

use chrono::Datelike;
use image::{png::PngEncoder, ColorType};
use plotters::prelude::*;
use rosu_v2::prelude::{MedalCompact, OsuError};
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Display medal stats for a user")]
#[usage("[username]")]
#[example("badewanne3", r#""im a fancy lad""#)]
#[aliases("ms")]
async fn medalstats(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let user_fut = request_user(&ctx, &name, None);
    let medals_fut = ctx.psql().get_medals();

    let (mut user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        (_, Err(why)) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
        (Err(why), _) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    user.medals
        .as_mut()
        .unwrap()
        .sort_unstable_by_key(|medal| medal.achieved_at);

    let graph = match graph(user.medals.as_ref().unwrap()) {
        Ok(bytes_option) => bytes_option,
        Err(why) => {
            unwind_error!(warn, why, "Error while calculating medal graph: {}");

            None
        }
    };

    let embed = MedalStatsEmbed::new(user, all_medals, graph.is_some())
        .build_owned()
        .build()?;

    // Send the embed
    let m = ctx.http.create_message(msg.channel_id).embed(embed)?;

    let response = if let Some(graph) = graph {
        m.attachment("medal_graph.png", graph).await?
    } else {
        m.await?
    };

    response.reaction_delete(&ctx, msg.author.id);

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

fn graph(medals: &[MedalCompact]) -> Result<Option<Vec<u8>>, Error> {
    static LEN: usize = W as usize * H as usize;
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3
    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;

        if medals.is_empty() {
            return Ok(None);
        }

        let mut medal_counter = Vec::with_capacity(medals.len());
        let mut counter = 0;

        for medal in medals {
            counter += 1;
            medal_counter.push((medal.achieved_at, counter));
        }

        let first = medals.first().unwrap().achieved_at;
        let last = medals.last().unwrap().achieved_at;

        let mut chart = ChartBuilder::on(&root)
            .margin_right(17)
            .caption("Medal history", ("sans-serif", 30))
            .x_label_area_size(30)
            .y_label_area_size(45)
            .build_cartesian_2d((first..last).monthly(), 0..counter)?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_labels(10)
            .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month()))
            .label_style(("sans-serif", 20))
            .draw()?;

        // Draw area
        chart.draw_series(
            AreaSeries::new(
                medal_counter.iter().map(|(date, count)| (*date, *count)),
                0,
                &BLUE.mix(0.2),
            )
            .border_style(&BLUE),
        )?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}

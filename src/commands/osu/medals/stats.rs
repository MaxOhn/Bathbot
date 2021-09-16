use crate::{
    embeds::{EmbedData, MedalStatsEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use chrono::Datelike;
use image::{png::PngEncoder, ColorType};
use plotters::prelude::*;
use rosu_v2::prelude::{MedalCompact, OsuError};
use std::sync::Arc;

#[command]
#[short_desc("Display medal stats for a user")]
#[usage("[username]")]
#[example("badewanne3", r#""im a fancy lad""#)]
#[aliases("ms")]
async fn medalstats(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match Args::check_user_mention(&ctx, arg).await {
                    Ok(Ok(name)) => Some(name),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.user_config(msg.author.id).await {
                    Ok(config) => config.osu_username,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
            };

            _medalstats(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => super::slash_medal(ctx, *command).await,
    }
}

pub(super) async fn _medalstats(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Name>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_fut = super::request_user(&ctx, &name, None);
    let medals_fut = ctx.psql().get_medals();

    let (mut user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        (_, Err(why)) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
        (Err(why), _) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    if let Some(ref mut medals) = user.medals {
        medals.sort_unstable_by_key(|medal| medal.achieved_at);
    }

    let graph = match graph(user.medals.as_ref().unwrap()) {
        Ok(bytes_option) => bytes_option,
        Err(why) => {
            unwind_error!(warn, why, "Error while calculating medal graph: {}");

            None
        }
    };

    let embed = MedalStatsEmbed::new(user, all_medals, graph.is_some())
        .into_builder()
        .build();

    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(ref graph) = graph {
        builder = builder.file("medal_graph.png", graph);
    }

    data.create_message(&ctx, builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

fn graph(medals: &[MedalCompact]) -> BotResult<Option<Vec<u8>>> {
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

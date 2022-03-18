use std::sync::Arc;

use chrono::{DateTime, Datelike, Utc};
use eyre::Report;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::prelude::*;
use rosu_v2::prelude::{GameMode, MedalCompact, OsuError, Username};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user, UserArgs},
    },
    database::OsuData,
    embeds::{EmbedData, MedalStatsEmbed},
    error::GraphError,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

#[command]
#[short_desc("Display medal stats for a user")]
#[usage("[username]")]
#[example("badewanne3", r#""im a fancy lad""#)]
#[aliases("ms")]
async fn medalstats(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
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

            _medalstats(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => super::slash_medal(ctx, *command).await,
    }
}

pub(super) async fn _medalstats(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Username>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::STD);
    let user_fut = get_user(&ctx, &user_args);
    let medals_fut = ctx.psql().get_medals();

    let (mut user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{name}` was not found");

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

    let graph = match graph(user.medals.as_ref().unwrap(), W, H) {
        Ok(bytes_option) => bytes_option,
        Err(err) => {
            warn!("{:?}", Report::new(err));

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

pub fn graph(medals: &[MedalCompact], w: u32, h: u32) -> Result<Option<Vec<u8>>, GraphError> {
    let len = (w * h * 3) as usize; // PIXEL_SIZE = 3
    let mut buf = vec![0; len];

    {
        let root = BitMapBackend::with_buffer(&mut buf, (w, h)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)?;

        if medals.is_empty() {
            return Ok(None);
        }

        let first = medals.first().unwrap().achieved_at;
        let last = medals.last().unwrap().achieved_at;

        let style: fn(RGBColor) -> ShapeStyle = |color| ShapeStyle {
            color: color.to_rgba(),
            filled: false,
            stroke_width: 1,
        };

        let mut chart = ChartBuilder::on(&root)
            .margin_right(22)
            .caption("Medal history", ("sans-serif", 30, &WHITE))
            .x_label_area_size(30)
            .y_label_area_size(45)
            .build_cartesian_2d((first..last).monthly(), 0..medals.len())?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_labels(10)
            .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month()))
            .label_style(("sans-serif", 20, &WHITE))
            .bold_line_style(&WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
            .draw()?;

        // Draw area
        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = style(RGBColor(0, 208, 138)).stroke_width(3);
        let counter = MedalCounter::new(medals);
        let series = AreaSeries::new(counter, 0, area_style).border_style(border_style);
        chart.draw_series(series)?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, w, h, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}

struct MedalCounter<'m> {
    count: usize,
    medals: &'m [MedalCompact],
}

impl<'m> MedalCounter<'m> {
    fn new(medals: &'m [MedalCompact]) -> Self {
        Self { count: 0, medals }
    }
}

impl Iterator for MedalCounter<'_> {
    type Item = (DateTime<Utc>, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let date = self.medals.first()?.achieved_at;
        self.count += 1;
        self.medals = &self.medals[1..];

        Some((date, self.count))
    }
}

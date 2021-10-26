use crate::{
    custom_client::SnipeCountryPlayer,
    database::OsuData,
    embeds::{CountrySnipeStatsEmbed, EmbedData},
    error::GraphError,
    util::{
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        CountryCode, MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use eyre::Report;
use image::{png::PngEncoder, ColorType};
use plotters::prelude::*;
use rosu_v2::prelude::{GameMode, OsuError};
use std::{cmp::Ordering::Equal, sync::Arc};

#[command]
#[short_desc("Snipe / #1 count related stats for a country")]
#[long_desc(
    "Some snipe / #1 count related stats for a country.\n\
    As argument, provide either `global`, or a country acronym, e.g. `be`.\n\
    If no country is specified, I will take the country of the linked user.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[country acronym]")]
#[example("fr", "global")]
#[aliases("css")]
#[bucket("snipe")]
async fn countrysnipestats(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let country_code = match args.next() {
                Some(arg) => {
                    if arg == "global" || arg == "world" {
                        Some("global".into())
                    } else if arg.len() == 2 && arg.is_ascii() {
                        let code = arg.to_ascii_uppercase();

                        if !ctx.contains_country(code.as_str()) {
                            let content =
                                format!("The country acronym `{}` is not supported :(", arg);

                            return msg.error(&ctx, content).await;
                        }

                        Some(code.into())
                    } else if let Some(code) = CountryCode::from_name(arg) {
                        if !code.snipe_supported(&ctx) {
                            let content = format!("The country `{}` is not supported :(", arg);

                            return msg.error(&ctx, content).await;
                        }

                        Some(code)
                    } else {
                        let content =
                            "The argument must be a country or country acronym of length two, e.g. `fr`";

                        return msg.error(&ctx, content).await;
                    }
                }
                None => None,
            };

            _countrysnipestats(ctx, CommandData::Message { msg, args, num }, country_code).await
        }
        CommandData::Interaction { command } => super::slash_snipe(ctx, *command).await,
    }
}

pub(super) async fn _countrysnipestats(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    country_code: Option<CountryCode>,
) -> BotResult<()> {
    let author_id = data.author()?.id;

    let country_code = match country_code {
        Some(code) => code,
        None => match ctx
            .psql()
            .get_user_osu(author_id)
            .await
            .map(|osu| osu.map(OsuData::into_username))
        {
            Ok(Some(name)) => {
                let user = match super::request_user(&ctx, &name, GameMode::STD).await {
                    Ok(user) => user,
                    Err(OsuError::NotFound) => {
                        let content = format!("User `{}` was not found", name);

                        return data.error(&ctx, content).await;
                    }
                    Err(why) => {
                        let _ = data.error(&ctx, OSU_API_ISSUE).await;

                        return Err(why.into());
                    }
                };

                if ctx.contains_country(user.country_code.as_str()) {
                    user.country_code.as_str().into()
                } else {
                    let content = format!(
                        "`{}`'s country {} is not supported :(",
                        user.username, user.country_code
                    );

                    return data.error(&ctx, content).await;
                }
            }
            Ok(None) => {
                let content =
                    "Since you're not linked, you must specify a country acronym, e.g. `fr`";

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        },
    };

    let client = &ctx.clients.custom;

    let (players, statistics) = {
        match tokio::try_join!(
            client.get_snipe_country(&country_code),
            client.get_country_statistics(&country_code),
        ) {
            Ok((players, statistics)) => (players, statistics),
            Err(why) => {
                let _ = data.error(&ctx, HUISMETBENEN_ISSUE).await;

                return Err(why.into());
            }
        }
    };

    let graph = match graphs(&players) {
        Ok(graph_option) => Some(graph_option),
        Err(err) => {
            warn!("{:?}", Report::new(err));

            None
        }
    };

    let country = ctx
        .get_country(country_code.as_str())
        .map(|name| (name, country_code));
    let embed_data = CountrySnipeStatsEmbed::new(country, statistics);

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

fn graphs(players: &[SnipeCountryPlayer]) -> Result<Vec<u8>, GraphError> {
    static LEN: usize = W as usize * H as usize;
    let mut pp: Vec<_> = players
        .iter()
        .map(|player| (&player.username, player.pp))
        .collect();
    pp.sort_unstable_by(|(_, pp1), (_, pp2)| pp2.partial_cmp(pp1).unwrap_or(Equal));
    pp.truncate(11);
    let mut count: Vec<_> = players
        .iter()
        .map(|player| (&player.username, player.count_first as i32))
        .collect();
    count.sort_unstable_by(|(_, c1), (_, c2)| c2.cmp(c1));
    count.truncate(11);
    let pp_max = pp
        .iter()
        .map(|(_, n)| *n)
        .fold(0.0_f32, |max, curr| max.max(curr));
    let count_max = count
        .iter()
        .map(|(_, n)| *n)
        .fold(0, |max, curr| max.max(curr));
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3
    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;
        let (left, right) = root.split_horizontally(W / 2);
        let mut chart = ChartBuilder::on(&left)
            .x_label_area_size(30)
            .y_label_area_size(60)
            .margin_right(15)
            .caption("Weighted pp from #1s", ("sans-serif", 30))
            .build_cartesian_2d(0..pp.len() - 1, 0.0..pp_max)?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_label_offset(30)
            .x_label_style(("sans-serif", 10))
            .x_label_formatter(&|idx| {
                if *idx < 10 {
                    pp[*idx].0.to_string()
                } else {
                    String::new()
                }
            })
            .draw()?;

        // Histogram bars
        chart.draw_series(
            Histogram::vertical(&chart)
                .style(BLUE.mix(0.5).filled())
                .data(
                    pp.iter()
                        .take(10)
                        .enumerate()
                        .map(|(idx, (_, n))| (idx, *n)),
                ),
        )?;

        // Count graph
        let mut chart = ChartBuilder::on(&right)
            .x_label_area_size(30)
            .y_label_area_size(35)
            .margin_right(15)
            .caption("#1 Count", ("sans-serif", 30))
            .build_cartesian_2d(0..count.len() - 1, 0..count_max)?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_label_offset(30)
            .x_label_style(("sans-serif", 10))
            .x_label_formatter(&|idx| {
                if *idx < 10 {
                    count[*idx].0.to_string()
                } else {
                    String::new()
                }
            })
            .draw()?;

        // Histogram bars
        chart.draw_series(
            Histogram::vertical(&chart)
                .style(RED.mix(0.5).filled())
                .data(
                    count
                        .iter()
                        .take(10)
                        .enumerate()
                        .map(|(idx, (_, n))| (idx, *n)),
                ),
        )?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;

    Ok(png_bytes)
}

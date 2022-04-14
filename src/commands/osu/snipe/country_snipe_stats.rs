use std::{borrow::Cow, cmp::Ordering::Equal, sync::Arc};

use command_macros::command;
use eyre::Report;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::prelude::*;
use rosu_v2::prelude::{GameMode, OsuError};

use crate::{
    commands::osu::UserArgs,
    core::commands::CommandOrigin,
    custom_client::SnipeCountryPlayer,
    database::OsuData,
    embeds::{CountrySnipeStatsEmbed, EmbedData},
    error::GraphError,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        CountryCode,
    },
    BotResult, Context,
};

use super::SnipeCountryStats;

#[command]
#[desc("Snipe / #1 count related stats for a country")]
#[help(
    "Some snipe / #1 count related stats for a country.\n\
    As argument, provide either `global`, or a country acronym, e.g. `be`.\n\
    If no country is specified, I will take the country of the linked user.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[country acronym]")]
#[examples("fr", "global")]
#[alias("css")]
#[group(Osu)]
async fn prefix_countrysnipestats(
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
) -> BotResult<()> {
    let args = SnipeCountryStats {
        country: args.next().map(Cow::from),
    };

    country_stats(ctx, msg.into(), args).await
}

pub(super) async fn country_stats(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: SnipeCountryStats<'_>,
) -> BotResult<()> {
    let author_id = orig.user_id()?;

    let country_code = match args.country {
        Some(country) => match CountryCode::from_name(&country) {
            Some(code) => code,
            None => {
                if country.len() == 2 {
                    CountryCode::from(country)
                } else {
                    let content = format!(
                        "Looks like `{country}` is neither a country name nor a country code"
                    );

                    return orig.error(&ctx, content).await;
                }
            }
        },
        None => match ctx
            .psql()
            .get_user_osu(author_id)
            .await
            .map(|osu| osu.map(OsuData::into_username))
        {
            Ok(Some(name)) => {
                let user_args = UserArgs::new(name.as_str(), GameMode::STD);

                let user = match ctx.redis().osu_user(&user_args).await {
                    Ok(user) => user,
                    Err(OsuError::NotFound) => {
                        let content = format!("User `{name}` was not found");

                        return orig.error(&ctx, content).await;
                    }
                    Err(err) => {
                        let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                        return Err(err.into());
                    }
                };

                user.country_code.as_str().into()
            }
            Ok(None) => {
                let content = "Since you're not linked, you must specify a country (code)";

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    // Check if huisemetbenen supports the country
    if !country_code.snipe_supported(&ctx) {
        let content = format!("The country code `{country_code}` is not supported :(",);

        return orig.error(&ctx, content).await;
    }

    let client = &ctx.client();

    let (players, statistics) = {
        match tokio::try_join!(
            client.get_snipe_country(&country_code),
            client.get_country_statistics(&country_code),
        ) {
            Ok((players, statistics)) => (players, statistics),
            Err(err) => {
                let _ = orig.error(&ctx, HUISMETBENEN_ISSUE).await;

                return Err(err.into());
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
        .get_country(country_code.as_ref())
        .map(|name| (name, country_code.into()));
    let embed_data = CountrySnipeStatsEmbed::new(country, statistics);

    // Sending the embed
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.attachment("stats_graph.png", bytes);
    }

    orig.create_message(&ctx, &builder).await?;

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
    png_encoder.write_image(&buf, W, H, ColorType::Rgb8)?;

    Ok(png_bytes)
}

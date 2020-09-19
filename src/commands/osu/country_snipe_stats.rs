use crate::{
    arguments::Args,
    custom_client::SnipeCountryPlayer,
    embeds::{CountrySnipeStatsEmbed, EmbedData},
    util::{constants::OSU_API_ISSUE, MessageExt, SNIPE_COUNTRIES},
    BotResult, Context,
};

use image::{png::PngEncoder, ColorType};
use plotters::prelude::*;
use rosu::models::GameMode;
use std::sync::Arc;
use twilight_model::channel::Message;

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
async fn countrysnipestats(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let country = match args.next() {
        Some(arg) => match arg {
            "global" | "world" => String::from("global"),
            _ => {
                if arg.len() != 2 || arg.chars().count() != 2 {
                    let content = "The argument must be a country acronym of length two, e.g. `fr`";
                    return msg.error(&ctx, content).await;
                }
                match SNIPE_COUNTRIES.get(&arg.to_uppercase()) {
                    Some(country) => country.snipe.clone(),
                    None => {
                        let content = "That country acronym is not supported :(";
                        return msg.error(&ctx, content).await;
                    }
                }
            }
        },
        None => match ctx.get_link(msg.author.id.0) {
            Some(name) => {
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
                match SNIPE_COUNTRIES.get(&user.country) {
                    Some(country) => country.snipe.to_owned(),
                    None => {
                        let content = format!(
                            "`{}`'s country {} is not supported :(",
                            user.username, user.country
                        );
                        return msg.error(&ctx, content).await;
                    }
                }
            }
            None => {
                let content =
                    "Since you're not linked, you must specify a country acronym, e.g. `fr`";
                return msg.error(&ctx, content).await;
            }
        },
    };
    let client = &ctx.clients.custom;
    let (players, unplayed, differences) = {
        match tokio::join!(
            client.get_snipe_country(&country),
            client.get_country_unplayed_amount(&country),
            client.get_country_biggest_difference(&country),
        ) {
            (Err(why), ..) | (_, Err(why), _) => {
                let content = "Some issue with the huismetbenen api, blame bade";
                let _ = msg.error(&ctx, content).await;
                return Err(why);
            }
            (.., Err(why)) if country != "global" => {
                let content = "Some issue with the huismetbenen api, blame bade";
                let _ = msg.error(&ctx, content).await;
                return Err(why);
            }
            (Ok(players), Ok(unplayed), differences) => (players, unplayed, differences.ok()),
        }
    };
    let graph = match graphs(&players) {
        Ok(graph_option) => Some(graph_option),
        Err(why) => {
            warn!("Error while creating snipe country graph: {}", why);
            None
        }
    };
    let country = SNIPE_COUNTRIES
        .iter()
        .find(|(_, c)| c.snipe == country)
        .map(|(_, country)| country);
    let data = CountrySnipeStatsEmbed::new(country, differences, unplayed as u64);

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

fn graphs(players: &[SnipeCountryPlayer]) -> BotResult<Vec<u8>> {
    static LEN: usize = W as usize * H as usize;
    let mut pp: Vec<_> = players
        .iter()
        .map(|player| (&player.username, player.pp))
        .collect();
    pp.sort_unstable_by(|(_, pp1), (_, pp2)| pp2.partial_cmp(pp1).unwrap());
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

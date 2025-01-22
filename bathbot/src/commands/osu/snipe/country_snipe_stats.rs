use std::{borrow::Cow, cmp::Ordering::Equal};

use bathbot_macros::command;
use bathbot_model::{Countries, SnipeCountryListOrder, SnipeCountryPlayer};
use bathbot_util::{constants::GENERAL_ISSUE, MessageBuilder};
use eyre::{ContextCompat, Report, Result, WrapErr};
use plotters::prelude::*;
use plotters_skia::SkiaBackend;
use rosu_v2::{
    model::GameMode,
    prelude::{CountryCode, OsuError},
    request::UserId,
};
use skia_safe::{surfaces, EncodedImageFormat};
use twilight_model::guild::Permissions;

use super::{SnipeCountryStats, SnipeGameMode};
use crate::{
    commands::osu::user_not_found,
    core::commands::CommandOrigin,
    embeds::{CountrySnipeStatsEmbed, EmbedData},
    manager::redis::osu::{UserArgs, UserArgsError},
    Context,
};

#[command]
#[desc("Snipe / #1 count related stats for a country")]
#[help(
    "Some snipe / #1 count related stats for a country.\n\
    As argument, provide an optional country acronym, e.g. `be`.\n\
    If no country is specified, I will take the country of the linked user.\n\
    Data for osu!standard originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[country acronym]")]
#[examples("fr")]
#[alias("css")]
#[group(Osu)]
async fn prefix_countrysnipestats(
    msg: &Message,
    mut args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipeCountryStats {
        mode: Some(SnipeGameMode::Osu),
        country: args.next().map(Cow::from),
    };

    country_stats(CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Snipe / #1 count related ctb stats for a country")]
#[help(
    "Some snipe / #1 count related ctb stats for a country.\n\
    As argument, provide an optional country acronym, e.g. `be`.\n\
    If no country is specified, I will take the country of the linked user.\n\
    Data for osu!catch originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[country acronym]")]
#[examples("fr")]
#[alias("cssc", "countrysnipestatscatch")]
#[group(Catch)]
async fn prefix_countrysnipestatsctb(
    msg: &Message,
    mut args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipeCountryStats {
        mode: Some(SnipeGameMode::Catch),
        country: args.next().map(Cow::from),
    };

    country_stats(CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Snipe / #1 count related mania stats for a country")]
#[help(
    "Some snipe / #1 count related mania stats for a country.\n\
    As argument, provide an optional country acronym, e.g. `be`.\n\
    If no country is specified, I will take the country of the linked user.\n\
    Data for osu!mania originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[country acronym]")]
#[examples("fr")]
#[alias("cssm")]
#[group(Mania)]
async fn prefix_countrysnipestatsmania(
    msg: &Message,
    mut args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipeCountryStats {
        mode: Some(SnipeGameMode::Mania),
        country: args.next().map(Cow::from),
    };

    country_stats(CommandOrigin::from_msg(msg, permissions), args).await
}

pub(super) async fn country_stats(
    orig: CommandOrigin<'_>,
    args: SnipeCountryStats<'_>,
) -> Result<()> {
    let config = match Context::user_config().with_osu_id(orig.user_id()?).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get user config"));
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let country_code = match args.country {
        Some(ref country) => match Countries::name(country).to_code() {
            Some(code) => CountryCode::from(code),
            None if country.len() == 2 => CountryCode::from(country.as_ref()),
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                return orig.error(content).await;
            }
        },
        None => match config.osu {
            Some(user_id) => {
                let user_args = UserArgs::user_id(user_id, mode);

                let user = match Context::redis().osu_user(user_args).await {
                    Ok(user) => user,
                    Err(UserArgsError::Osu(OsuError::NotFound)) => {
                        let content = user_not_found(UserId::Id(user_id)).await;

                        return orig.error(content).await;
                    }
                    Err(err) => {
                        let _ = orig.error(GENERAL_ISSUE).await;
                        let err = Report::new(err).wrap_err("Failed to get user");

                        return Err(err);
                    }
                };

                user.country_code.as_str().into()
            }
            None => {
                let content = "Since you're not linked, you must specify a country (code)";

                return orig.error(content).await;
            }
        },
    };

    // Check if huisemetbenen supports the country
    if !Context::huismetbenen()
        .is_supported(country_code.as_str(), mode)
        .await
    {
        let content = format!("The country code `{country_code}` is not supported :(",);

        return orig.error(content).await;
    }

    let client = Context::client();

    let players_fut =
        client.get_snipe_country(&country_code, SnipeCountryListOrder::WeightedPp, mode);
    let stats_fut = client.get_country_statistics(&country_code, mode);

    let (players, statistics) = {
        match tokio::try_join!(players_fut, stats_fut,) {
            Ok((players, statistics)) => (players, statistics),
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get country data"));
            }
        }
    };

    let graph = match graphs(&players) {
        Ok(graph_option) => Some(graph_option),
        Err(err) => {
            warn!(?err, "Failed to create graph");

            None
        }
    };

    let country = Countries::code(&country_code)
        .to_name()
        .map(|name| (name, country_code));

    let embed_data = CountrySnipeStatsEmbed::new(country, statistics);

    // Sending the embed
    let embed = embed_data.build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.attachment("stats_graph.png", bytes);
    }

    orig.create_message(builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

fn graphs(players: &[SnipeCountryPlayer]) -> Result<Vec<u8>> {
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

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let (left, right) = root.split_horizontally(W / 2);

        let mut chart = ChartBuilder::on(&left)
            .x_label_area_size(30)
            .y_label_area_size(60)
            .margin_right(15)
            .caption("Weighted pp from #1s", ("sans-serif", 30, &WHITE))
            .build_cartesian_2d(0..pp.len() - 1, 0.0..pp_max)
            .wrap_err("failed to build left chart")?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_label_offset(30)
            .label_style(("sans-serif", 12, &WHITE))
            .x_label_formatter(&|idx| {
                if *idx < 10 {
                    pp[*idx].0.to_string()
                } else {
                    String::new()
                }
            })
            .draw()
            .wrap_err("failed to draw left mesh")?;

        // Histogram bars
        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();

        let iter = pp
            .iter()
            .take(10)
            .enumerate()
            .map(|(idx, (_, n))| (idx, *n));

        chart
            .draw_series(Histogram::vertical(&chart).style(area_style).data(iter))
            .wrap_err("failed to draw left series")?;

        // Count graph
        let mut chart = ChartBuilder::on(&right)
            .x_label_area_size(30)
            .y_label_area_size(35)
            .margin_right(15)
            .caption("#1 Count", ("sans-serif", 30, &WHITE))
            .build_cartesian_2d(0..count.len() - 1, 0..count_max)
            .wrap_err("failed to build right chart")?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_label_offset(30)
            .label_style(("sans-serif", 12, &WHITE))
            .x_label_formatter(&|idx| {
                if *idx < 10 {
                    count[*idx].0.to_string()
                } else {
                    String::new()
                }
            })
            .draw()
            .wrap_err("failed to draw right mesh")?;

        // Histogram bars
        let iter = count
            .iter()
            .take(10)
            .enumerate()
            .map(|(idx, (_, n))| (idx, *n));

        chart
            .draw_series(Histogram::vertical(&chart).style(area_style).data(iter))
            .wrap_err("failed to draw right series")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

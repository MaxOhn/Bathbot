use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{Date, DateTime, Duration, Utc};
use eyre::Report;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use itertools::Itertools;
use plotters::{
    coord::{
        ranged1d::SegmentedCoord,
        types::{RangedCoordusize, RangedSlice},
        Shift,
    },
    prelude::*,
};
use rosu_v2::prelude::{GameMode, OsuError, Username};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user, UserArgs},
    },
    custom_client::SnipeRecent,
    database::OsuData,
    embeds::{EmbedData, SnipedEmbed},
    error::GraphError,
    util::{
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

#[command]
#[short_desc("Sniped users of the last 8 weeks")]
#[long_desc(
    "Sniped users of the last 8 weeks.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("snipes")]
#[bucket("snipe")]
async fn sniped(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
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

            let data = CommandData::Message { msg, args, num };

            _sniped(ctx, data, name).await
        }
        CommandData::Interaction { command } => super::slash_snipe(ctx, *command).await,
    }
}

pub(super) async fn _sniped(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Username>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::STD);

    let mut user = match get_user(&ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = GameMode::STD;

    let client = &ctx.clients.custom;
    let now = Utc::now();

    let (sniper, snipee) = if ctx.contains_country(user.country_code.as_str()) {
        let sniper_fut = client.get_national_snipes(&user, true, now - Duration::weeks(8), now);
        let snipee_fut = client.get_national_snipes(&user, false, now - Duration::weeks(8), now);

        match tokio::try_join!(sniper_fut, snipee_fut) {
            Ok((mut sniper, snipee)) => {
                sniper.retain(|score| score.sniped.is_some());

                (sniper, snipee)
            }
            Err(why) => {
                let _ = data.error(&ctx, HUISMETBENEN_ISSUE).await;

                return Err(why.into());
            }
        }
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return data.error(&ctx, content).await;
    };

    let graph = match graphs(user.username.as_str(), &sniper, &snipee, W, H) {
        Ok(graph_option) => graph_option,
        Err(err) => {
            warn!("{:?}", Report::new(err));

            None
        }
    };

    let embed_data = SnipedEmbed::new(user, sniper, snipee);

    // Sending the embed
    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.file("sniped_graph.png", bytes);
    }

    data.create_message(&ctx, builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

pub fn graphs(
    name: &str,
    sniper: &[SnipeRecent],
    snipee: &[SnipeRecent],
    w: u32,
    h: u32,
) -> Result<Option<Vec<u8>>, GraphError> {
    if sniper.is_empty() && snipee.is_empty() {
        return Ok(None);
    }

    let len = (w * h) as usize;
    let mut buf = vec![0; len * 3];

    {
        let root = BitMapBackend::with_buffer(&mut buf, (w, h)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)?;

        match (sniper.is_empty(), snipee.is_empty()) {
            (false, true) => draw_sniper(&root, name, sniper)?,
            (true, false) => draw_snipee(&root, name, snipee)?,
            (false, false) => {
                let (left, right) = root.split_horizontally(w / 2);
                draw_sniper(&left, name, sniper)?;
                draw_snipee(&right, name, snipee)?
            }
            (true, true) => unreachable!(),
        }
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, w, h, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}

type ContextType<'a> = Cartesian2d<SegmentedCoord<RangedSlice<'a, Date<Utc>>>, RangedCoordusize>;
type DrawingError<DB> = Result<(), DrawingAreaErrorKind<<DB as DrawingBackend>::ErrorType>>;
type PrepareResult<'a> = (Vec<Date<Utc>>, Vec<(&'a str, Vec<usize>)>);

fn draw_sniper<DB: DrawingBackend>(
    root: &DrawingArea<DB, Shift>,
    name: &str,
    sniper: &[SnipeRecent],
) -> DrawingError<DB> {
    let (dates, sniper) = prepare_sniper(sniper);

    let max = sniper
        .iter()
        .map(|(_, v)| v.last().copied())
        .max()
        .flatten()
        .unwrap_or(0);

    let mut chart = ChartBuilder::on(root)
        .x_label_area_size(30)
        .y_label_area_size(35)
        .margin_right(5)
        .caption(format!("Sniped by {name}"), ("sans-serif", 25, &WHITE))
        .build_cartesian_2d((&dates[..]).into_segmented(), 0..max + 1)?;

    draw_mesh(&mut chart)?;

    for (i, (name, values)) in sniper.into_iter().enumerate() {
        draw_histogram_block(i, name, &values, &dates, &mut chart)?;
    }

    draw_legend(&mut chart)?;

    Ok(())
}

fn draw_snipee<DB: DrawingBackend>(
    root: &DrawingArea<DB, Shift>,
    name: &str,
    snipee: &[SnipeRecent],
) -> DrawingError<DB> {
    let (dates, snipee) = prepare_snipee(snipee);

    let max = snipee
        .iter()
        .map(|(_, v)| v.last().copied())
        .max()
        .flatten()
        .unwrap_or(0);

    let mut chart = ChartBuilder::on(root)
        .x_label_area_size(30)
        .y_label_area_size(35)
        .margin_right(5)
        .caption(format!("Sniped {name}"), ("sans-serif", 25, &WHITE))
        .build_cartesian_2d((&dates[..]).into_segmented(), 0..max + 1)?;

    draw_mesh(&mut chart)?;

    for (i, (name, values)) in snipee.into_iter().enumerate() {
        draw_histogram_block(i, name, &values, &dates, &mut chart)?;
    }

    draw_legend(&mut chart)?;

    Ok(())
}

fn draw_mesh<DB: DrawingBackend>(
    chart: &mut ChartContext<'_, DB, ContextType<'_>>,
) -> DrawingError<DB> {
    chart
        .configure_mesh()
        .disable_x_mesh()
        .x_label_formatter(&|date: &SegmentValue<&Date<Utc>>| match date {
            SegmentValue::CenterOf(date) | SegmentValue::Exact(date) => {
                date.format("%Y-%m-%d").to_string()
            }
            _ => unreachable!(),
        })
        .label_style(("sans-serif", 15, &WHITE))
        .bold_line_style(&WHITE.mix(0.3))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 20_i32, FontStyle::Bold, &WHITE))
        .draw()
}

fn draw_histogram_block<'a, DB: DrawingBackend + 'a>(
    i: usize,
    name: &str,
    values: &[usize],
    dates: &'a [Date<Utc>],
    chart: &mut ChartContext<'a, DB, ContextType<'a>>,
) -> DrawingError<DB> {
    // Draw block
    let data = values
        .iter()
        .enumerate()
        .map(|(i, count)| (&dates[i], *count));

    let color = HSLColor(i as f64 * 0.1, 0.5, 0.5);

    let series = Histogram::vertical(&chart)
        .data(data)
        .style(color.mix(0.75).filled());

    chart
        .draw_series(series)?
        .label(name)
        .legend(move |(x, y)| Circle::new((x, y), 4, color.filled()));

    // Draw border
    let data = values
        .iter()
        .enumerate()
        .map(|(i, count)| (&dates[i], *count));

    let color = HSLColor(i as f64 * 0.1, 0.5, 0.3);
    let series = Histogram::vertical(&chart).data(data).style(color);
    chart.draw_series(series)?;

    Ok(())
}

fn draw_legend<'a, DB: DrawingBackend + 'a>(
    chart: &mut ChartContext<'a, DB, ContextType<'_>>,
) -> DrawingError<DB> {
    chart
        .configure_series_labels()
        .border_style(WHITE.mix(0.6).stroke_width(1))
        .background_style(RGBColor(7, 23, 17))
        .position(SeriesLabelPosition::UpperLeft)
        .legend_area_size(13)
        .label_font(("sans-serif", 15, FontStyle::Bold, &WHITE))
        .draw()?;

    Ok(())
}

fn prepare_snipee(scores: &[SnipeRecent]) -> PrepareResult<'_> {
    let mut total = HashMap::new();

    for score in scores {
        *total.entry(score.sniper.as_str()).or_insert(0) += 1;
    }

    let mut final_order: Vec<_> = total.into_iter().collect();
    final_order.sort_unstable_by_key(|(_, c)| Reverse(*c));
    final_order.truncate(10);

    let names: HashSet<_> = final_order.iter().map(|(name, _)| *name).collect();

    let categorized: Vec<_> = scores
        .iter()
        .scan(Utc::now() - Duration::weeks(7), |state, score| {
            if !names.contains(score.sniper.as_str()) {
                return Some(None);
            }

            if score.date > *state {
                while score.date > *state {
                    *state = *state + Duration::weeks(1);
                }
            }

            Some(Some((score.sniper.as_str(), *state)))
        })
        .flatten()
        .collect();

    finish_preparing(names, categorized)
}

fn prepare_sniper(scores: &[SnipeRecent]) -> PrepareResult<'_> {
    let mut total = HashMap::new();

    for sniped in scores.iter().filter_map(|score| score.sniped.as_deref()) {
        *total.entry(sniped).or_insert(0) += 1;
    }

    let mut final_order: Vec<_> = total.into_iter().collect();
    final_order.sort_unstable_by_key(|(_, c)| Reverse(*c));
    final_order.truncate(10);

    let names: HashSet<_> = final_order.iter().map(|(name, _)| *name).collect();

    let categorized: Vec<_> = scores
        .iter()
        .filter(|score| score.sniped.is_some())
        .scan(Utc::now() - Duration::weeks(7), |state, score| {
            if !names.contains(score.sniped.as_deref().unwrap()) {
                return Some(None);
            }

            if score.date > *state {
                while score.date > *state {
                    *state = *state + Duration::weeks(1);
                }
            }

            Some(Some((score.sniped.as_deref().unwrap(), *state)))
        })
        .flatten()
        .collect();

    finish_preparing(names, categorized)
}

fn finish_preparing<'a>(
    names: HashSet<&'a str>,
    categorized: Vec<(&'a str, DateTime<Utc>)>,
) -> PrepareResult<'a> {
    let (dates, counts): (Vec<_>, Vec<_>) = categorized
        .into_iter()
        .group_by(|(_, date)| *date)
        .into_iter()
        .map(|(date, group)| {
            let mut counts = HashMap::new();

            for (name, _) in group {
                *counts.entry(name).or_insert(0) += 1;
            }

            (date.date(), counts)
        })
        .unzip();

    let mut total: HashMap<_, _> = names.into_iter().map(|name| (name, Vec::new())).collect();

    for counts in counts {
        for (name, values) in total.iter_mut() {
            values.push(counts.get(name).copied().unwrap_or(0));
        }
    }

    for values in total.values_mut() {
        for i in 1..values.len() {
            values[i] += values[i - 1];
        }
    }

    let mut total: Vec<_> = total.into_iter().collect();
    total.sort_unstable_by_key(|(_, values)| Reverse(values.last().copied()));

    for (i, j) in (1..total.len()).zip(0..total.len() - 1).rev() {
        for k in 0..dates.len() {
            total[j].1[k] += total[i].1[k];
        }
    }

    (dates, total)
}

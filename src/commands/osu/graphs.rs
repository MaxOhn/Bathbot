use std::sync::Arc;

use chrono::{Duration, FixedOffset, Timelike, Utc};
use eyre::Report;
use image::{png::PngEncoder, ColorType, ImageEncoder};
use plotters::{
    prelude::{
        AreaSeries, BitMapBackend, ChartBuilder, Circle, IntoDrawingArea, IntoSegmentedCoord,
        Rectangle, SegmentValue, SeriesLabelPosition,
    },
    style::{Color, RGBColor, ShapeStyle, BLACK, GREEN, RED, WHITE},
};
use plotters_backend::FontStyle;
use rosu_v2::prelude::{GameMode, OsuError, Score, User};
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

use crate::{
    commands::{
        osu::{get_user, get_user_and_scores, ScoreArgs, UserArgs},
        parse_discord, parse_mode_option, MyCommand, MyCommandOption,
    },
    core::{commands::CommandData, Context},
    database::UserConfig,
    embeds::{Author, EmbedBuilder, EmbedData, Footer, GraphEmbed},
    error::{Error, GraphError},
    util::{
        constants::{
            common_literals::{DISCORD, MODE, NAME},
            GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE, OSU_BASE,
        },
        numbers::{with_comma_float, with_comma_int},
        osu::flag_url,
        CountryCode, InteractionExt, MessageBuilder, MessageExt,
    },
    BotResult,
};

use super::{option_discord, option_mode, option_name};

async fn graph(ctx: Arc<Context>, data: CommandData<'_>, args: GraphArgs) -> BotResult<()> {
    let GraphArgs { config, kind } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), mode);

    let tuple_option = match kind {
        GraphKind::MedalProgression => medals_graph(&ctx, &data, &name, &user_args).await?,
        GraphKind::PlaycountReplays => {
            playcount_replays_graph(&ctx, &data, &name, &user_args).await?
        }
        GraphKind::RankProgression => rank_graph(&ctx, &data, &name, &user_args).await?,
        GraphKind::ScoreTime => {
            // Handle distinctly because it has a footer due to the timezone
            let tuple_option = score_time_graph(&ctx, &data, &name, user_args).await?;

            let (user, graph, tz) = match tuple_option {
                Some(tuple) => tuple,
                None => return Ok(()),
            };

            let author = {
                let stats = user.statistics.as_ref().expect("no statistics on user");

                let text = format!(
                    "{name}: {pp}pp (#{global} {country}{national})",
                    name = user.username,
                    pp = with_comma_float(stats.pp),
                    global = with_comma_int(stats.global_rank.unwrap_or(0)),
                    country = user.country_code,
                    national = stats.country_rank.unwrap_or(0)
                );

                let url = format!("{OSU_BASE}users/{}/{}", user.user_id, user.mode);
                let icon = flag_url(user.country_code.as_str());

                Author::new(text).url(url).icon_url(icon)
            };

            let footer = Footer::new(format!("Considering timezone UTC{tz}"));

            let embed = EmbedBuilder::new()
                .author(author)
                .footer(footer)
                .image("attachment://graph.png")
                .build();

            let builder = MessageBuilder::new().embed(embed).file("graph.png", &graph);
            data.create_message(&ctx, builder).await?;

            return Ok(());
        }
        GraphKind::Sniped => sniped_graph(&ctx, &data, &name, &user_args).await?,
        GraphKind::SnipeCount => snipe_count_graph(&ctx, &data, &name, &user_args).await?,
    };

    let (user, graph) = match tuple_option {
        Some(tuple) => tuple,
        None => return Ok(()),
    };

    let embed = GraphEmbed::new(&user).into_builder().build();
    let builder = MessageBuilder::new().embed(embed).file("graph.png", &graph);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

async fn medals_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let mut user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    if let Some(ref mut medals) = user.medals {
        medals.sort_unstable_by_key(|medal| medal.achieved_at);
    }

    let bytes = match super::medals::stats::graph(user.medals.as_ref().unwrap()) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` does not have any medals");
            let builder = MessageBuilder::new().embed(content);
            data.create_message(ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn playcount_replays_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let mut user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let bytes = match super::profile::graphs(ctx, &mut user).await {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` does not have enough playcount data points");
            let builder = MessageBuilder::new().embed(content);
            data.create_message(ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn rank_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    fn draw_graph(user: &User) -> Result<Option<Vec<u8>>, GraphError> {
        const W: u32 = 750;
        const H: u32 = 400;
        const LEN: usize = (W * H) as usize * 3;

        let mut buf = vec![0; LEN];

        let history = match user.rank_history {
            Some(ref history) if history.is_empty() => return Ok(None),
            Some(ref history) => history,
            None => return Ok(None),
        };

        let history_len = history.len();

        let mut min = u32::MAX;
        let mut max = 0;

        let mut min_idx = 0;
        let mut max_idx = 0;

        for (i, &rank) in history.iter().enumerate() {
            if rank < min {
                min = rank;
                min_idx = i;

                if rank > max {
                    max = rank;
                    max_idx = i;
                }
            } else if rank > max {
                max = rank;
                max_idx = i;
            }
        }

        let y_label_area_size = if max > 1_000_000 {
            75
        } else if max > 10_000 {
            65
        } else if max > 100 {
            50
        } else {
            40
        };

        let (min, max) = (-(max as i32), -(min as i32));

        {
            let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
            let background = RGBColor(19, 43, 33);
            root.fill(&background)?;

            let circle_style: fn(RGBColor) -> ShapeStyle = |color| ShapeStyle {
                color: color.to_rgba(),
                filled: false,
                stroke_width: 1,
            };

            let mut chart = ChartBuilder::on(&root)
                .x_label_area_size(40)
                .y_label_area_size(y_label_area_size)
                .margin(10)
                .margin_left(6)
                .build_cartesian_2d(0_u32..history_len.saturating_sub(1) as u32, min..max)?;

            chart
                .configure_mesh()
                .disable_y_mesh()
                .x_labels(20)
                .x_desc("Days ago")
                .x_label_formatter(&|x| format!("{}", 90 - *x))
                .y_label_formatter(&|y| format!("{}", -*y))
                .y_desc("Rank")
                .label_style(("sans-serif", 15, &WHITE))
                .bold_line_style(&WHITE.mix(0.3))
                .axis_style(RGBColor(7, 18, 14))
                .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
                .draw()?;

            let data = (0..).zip(history.iter().map(|rank| -(*rank as i32)));

            let area_style = RGBColor(2, 186, 213).mix(0.8).filled();
            let series = AreaSeries::new(data, min, area_style);
            chart.draw_series(series)?;

            let max_coords = (min_idx as u32, max);
            let circle = Circle::new(max_coords, 9_u32, circle_style(GREEN));

            chart
                .draw_series(std::iter::once(circle))?
                .label(format!("Peak: #{}", with_comma_int(-max)))
                .legend(|(x, y)| Circle::new((x, y), 5_u32, circle_style(GREEN)));

            let min_coords = (max_idx as u32, min);
            let circle = Circle::new(min_coords, 9_u32, circle_style(RED));

            chart
                .draw_series(std::iter::once(circle))?
                .label(format!("Worst: #{}", with_comma_int(-min)))
                .legend(|(x, y)| Circle::new((x, y), 5_u32, circle_style(RED)));

            let position = if min_idx <= 70 {
                SeriesLabelPosition::UpperRight
            } else if max_idx > 70 {
                SeriesLabelPosition::UpperLeft
            } else {
                SeriesLabelPosition::LowerRight
            };

            chart
                .configure_series_labels()
                .border_style(BLACK.stroke_width(2))
                .background_style(&RGBColor(192, 192, 192))
                .position(position)
                .legend_area_size(13)
                .label_font(("sans-serif", 15, FontStyle::Bold))
                .draw()?;
        }

        // Encode buf to png
        let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
        let png_encoder = PngEncoder::new(&mut png_bytes);
        png_encoder.write_image(&buf, W, H, ColorType::Rgb8)?;

        Ok(Some(png_bytes))
    }

    let bytes = match draw_graph(&user) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` has no available rank data :(");
            let _ = data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn score_time_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>, FixedOffset)>> {
    let score_args = ScoreArgs::top(100);

    let (user, scores) = match get_user_and_scores(ctx, user_args, &score_args).await {
        Ok(tuple) => tuple,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    fn draw_graph(scores: &[Score], tz: &FixedOffset) -> Result<Vec<u8>, GraphError> {
        const W: u32 = 750;
        const H: u32 = 400;
        const LEN: usize = (W * H) as usize * 3;

        let mut hours = [0_u32; 24];

        for score in scores {
            hours[score.created_at.with_timezone(tz).hour() as usize] += 1;
        }

        let max = hours.iter().max().copied().unwrap_or(0);
        let mut buf = vec![0; LEN];

        {
            let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
            let background = RGBColor(19, 43, 33);
            root.fill(&background)?;

            let mut chart = ChartBuilder::on(&root)
                .x_label_area_size(40)
                .y_label_area_size(40)
                .margin(5)
                .build_cartesian_2d((0_u32..23_u32).into_segmented(), 0u32..max + 1)?;

            chart
                .configure_mesh()
                .disable_x_mesh()
                .x_labels(24)
                .x_desc("Hour of the day")
                .y_desc("#  of  plays  set")
                .label_style(("sans-serif", 15, &WHITE))
                .bold_line_style(&WHITE.mix(0.3))
                .axis_style(RGBColor(7, 18, 14))
                .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
                .draw()?;

            let counts = ScoreHourCounts::new(hours);
            chart.draw_series(counts)?;
        }

        // Encode buf to png
        let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
        let png_encoder = PngEncoder::new(&mut png_bytes);
        png_encoder.write_image(&buf, W, H, ColorType::Rgb8)?;

        Ok(png_bytes)
    }

    let tz = CountryCode::from(user.country_code.clone()).timezone();

    let bytes = match draw_graph(&scores, &tz) {
        Ok(graph) => graph,
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes, tz)))
}

async fn sniped_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let (sniper, snipee) = if ctx.contains_country(user.country_code.as_str()) {
        let now = Utc::now();
        let sniper_fut =
            ctx.clients
                .custom
                .get_national_snipes(&user, true, now - Duration::weeks(8), now);
        let snipee_fut =
            ctx.clients
                .custom
                .get_national_snipes(&user, false, now - Duration::weeks(8), now);

        match tokio::try_join!(sniper_fut, snipee_fut) {
            Ok((mut sniper, snipee)) => {
                sniper.retain(|score| score.sniped.is_some());

                (sniper, snipee)
            }
            Err(err) => {
                let _ = data.error(ctx, HUISMETBENEN_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        data.error(ctx, content).await?;

        return Ok(None);
    };

    let bytes = match super::snipe::sniped::graphs(user.username.as_str(), &sniper, &snipee) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content =
                format!("`{name}` was neither sniped nor sniped other people in the last 8 weeks");
            let builder = MessageBuilder::new().embed(content);
            data.create_message(ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn snipe_count_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let player = if ctx.contains_country(user.country_code.as_str()) {
        let player_fut = ctx
            .clients
            .custom
            .get_snipe_player(&user.country_code, user.user_id);

        match player_fut.await {
            Ok(counts) => counts,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to retrieve snipe player");
                warn!("{report:?}");
                let content = format!("`{name}` has never had any national #1s");
                let builder = MessageBuilder::new().embed(content);
                data.create_message(&ctx, builder).await?;

                return Ok(None);
            }
        }
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        data.error(&ctx, content).await?;

        return Ok(None);
    };

    let graph_result = super::snipe::player_snipe_stats::graphs(
        &player.count_first_history,
        &player.count_sr_spread,
    );

    let bytes = match graph_result {
        Ok(graph) => graph,
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

struct ScoreHourCounts {
    hours: [u32; 24],
    idx: usize,
}

impl ScoreHourCounts {
    fn new(hours: [u32; 24]) -> Self {
        Self { hours, idx: 0 }
    }
}

impl Iterator for ScoreHourCounts {
    type Item = Rectangle<(SegmentValue<u32>, u32)>;

    fn next(&mut self) -> Option<Self::Item> {
        let count = *self.hours.get(self.idx)?;
        let hour = self.idx as u32;
        self.idx += 1;

        let top_left = (SegmentValue::Exact(hour), count);
        let bot_right = (SegmentValue::Exact(hour + 1), 0);

        let style = RGBColor(2, 186, 213).mix(0.8).filled();

        let mut rect = Rectangle::new([top_left, bot_right], style);
        rect.set_margin(0, 0, 2, 2);

        Some(rect)
    }
}

struct GraphArgs {
    config: UserConfig,
    kind: GraphKind,
}

enum GraphKind {
    MedalProgression,
    PlaycountReplays,
    RankProgression,
    ScoreTime,
    Sniped,
    SnipeCount,
}

pub async fn slash_graph(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let (subcommand, options) = command
        .data
        .options
        .pop()
        .and_then(|option| match option.value {
            CommandOptionValue::SubCommand(options) => Some((option.name, options)),
            _ => None,
        })
        .ok_or(Error::InvalidCommandOptions)?;

    let mut config = ctx.user_config(command.user_id()?).await?;

    let kind = match subcommand.as_str() {
        "medals" => GraphKind::MedalProgression,
        "playcount_replays" => GraphKind::PlaycountReplays,
        "rank" => GraphKind::RankProgression,
        "score_time" => GraphKind::ScoreTime,
        "sniped" => GraphKind::Sniped,
        "snipe_count" => GraphKind::SnipeCount,
        _ => return Err(Error::InvalidCommandOptions),
    };

    for option in options {
        match option.value {
            CommandOptionValue::String(value) => match option.name.as_str() {
                NAME => config.osu = Some(value.into()),
                MODE => config.mode = parse_mode_option(&value),
                _ => return Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::User(value) => match option.name.as_str() {
                DISCORD => match parse_discord(&ctx, value).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return command.error(&ctx, content).await,
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    graph(ctx, command.into(), GraphArgs { config, kind }).await
}

pub fn define_graph() -> MyCommand {
    let medals = MyCommandOption::builder("medals", "Display a user's medal progress over time")
        .subcommand(vec![option_name(), option_discord()]);

    let playcount_replays_description = "Display a user's playcount and replays watched over time";

    let playcount_replays =
        MyCommandOption::builder("playcount_replays", playcount_replays_description)
            .subcommand(vec![option_name(), option_discord()]);

    let rank = MyCommandOption::builder("rank", "Display a user's rank progression over time")
        .subcommand(vec![option_mode(), option_name(), option_discord()]);

    let score_time_description = "Display at what times a user set their top scores";

    let score_time = MyCommandOption::builder("score_time", score_time_description)
        .subcommand(vec![option_mode(), option_name(), option_discord()]);

    let sniped = MyCommandOption::builder("sniped", "Display sniped users of the past 8 weeks")
        .subcommand(vec![option_name(), option_discord()]);

    let snipe_count_description = "Display how a user's national #1 count progressed";

    let snipe_count = MyCommandOption::builder("snipe_count", snipe_count_description)
        .subcommand(vec![option_name(), option_discord()]);

    let subcommands = vec![
        medals,
        playcount_replays,
        rank,
        score_time,
        sniped,
        snipe_count,
    ];

    MyCommand::new("graph", "Display graphs about some data").options(subcommands)
}

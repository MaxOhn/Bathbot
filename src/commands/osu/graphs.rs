use std::{fmt::Write, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result, WrapErr};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use plotters::{
    prelude::{
        AreaSeries, BitMapBackend, ChartBuilder, Circle, EmptyElement, IntoDrawingArea,
        IntoSegmentedCoord, PointSeries, Rectangle, SegmentValue, SeriesLabelPosition,
    },
    style::{Color, RGBColor, ShapeStyle, BLACK, GREEN, RED, WHITE},
};
use plotters_backend::FontStyle;
use rosu_v2::prelude::{GameMode, OsuError, Score, User};
use time::{Duration, OffsetDateTime, UtcOffset};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{
        osu::{get_user, get_user_and_scores, ProfileGraphFlags, ScoreArgs, UserArgs},
        GameModeOption, ShowHideOption,
    },
    core::{commands::CommandOrigin, Context},
    embeds::{EmbedData, GraphEmbed},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        numbers::with_comma_int,
        CountryCode, InteractionCommandExt, Monthly,
    },
};

use super::{player_snipe_stats, profile, require_link, sniped, ProfileGraphParams};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "graph")]
/// Display graphs about some user data
pub enum Graph {
    #[command(name = "medals")]
    Medals(GraphMedals),
    #[command(name = "playcount_replays")]
    PlaycountReplays(GraphPlaycountReplays),
    #[command(name = "rank")]
    Rank(GraphRank),
    #[command(name = "sniped")]
    Sniped(GraphSniped),
    #[command(name = "snipe_count")]
    SnipeCount(GraphSnipeCount),
    #[command(name = "top")]
    Top(GraphTop),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "medals")]
/// Display a user's medal progress over time
pub struct GraphMedals {
    /// Specify a username
    name: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "playcount_replays")]
/// Display a user's playcount and replays watched over time
pub struct GraphPlaycountReplays {
    /// Specify a username
    name: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
    /// Specify if the playcount curve should be included
    playcount: Option<ShowHideOption>,
    /// Specify if the replay curve should be included
    replays: Option<ShowHideOption>,
    /// Specify if the badges should be included
    badges: Option<ShowHideOption>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "rank")]
/// Display a user's rank progression over time
pub struct GraphRank {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "sniped")]
/// Display sniped users of the past 8 weeks
pub struct GraphSniped {
    /// Specify a username
    name: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "snipe_count")]
/// Display how a user's national #1 count progressed
pub struct GraphSnipeCount {
    /// Specify a username
    name: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "top",
    help = "Display a user's top scores pp.\n\
    The timezone option is only relevant for the `Time` order."
)]
/// Display a user's top scores pp
pub struct GraphTop {
    /// Choose by which order the scores should be sorted, defaults to index
    order: GraphTopOrder,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    /// Specify a timezone (only relevant when ordered by `Time`)
    timezone: Option<GraphTopTimezone>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandOption, CreateOption)]
pub enum GraphTopOrder {
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Index", value = "index")]
    Index,
    #[option(name = "Time", value = "time")]
    Time,
}

#[derive(CommandOption, CreateOption)]
pub enum GraphTopTimezone {
    #[option(name = "UTC-12", value = "-12")]
    M12 = -12,
    #[option(name = "UTC-11", value = "-11")]
    M11 = -11,
    #[option(name = "UTC-10", value = "-10")]
    M10 = -10,
    #[option(name = "UTC-9", value = "-9")]
    M9 = -9,
    #[option(name = "UTC-8", value = "-8")]
    M8 = -8,
    #[option(name = "UTC-7", value = "-7")]
    M7 = -7,
    #[option(name = "UTC-6", value = "-6")]
    M6 = -6,
    #[option(name = "UTC-5", value = "-5")]
    M5 = -5,
    #[option(name = "UTC-4", value = "-4")]
    M4 = -4,
    #[option(name = "UTC-3", value = "-3")]
    M3 = -3,
    #[option(name = "UTC-2", value = "-2")]
    M2 = -2,
    #[option(name = "UTC-1", value = "-1")]
    M1 = -1,
    #[option(name = "UTC+0", value = "0")]
    P0 = 0,
    #[option(name = "UTC+1", value = "1")]
    P1 = 1,
    #[option(name = "UTC+2", value = "2")]
    P2 = 2,
    #[option(name = "UTC+3", value = "3")]
    P3 = 3,
    #[option(name = "UTC+4", value = "4")]
    P4 = 4,
    #[option(name = "UTC+5", value = "5")]
    P5 = 5,
    #[option(name = "UTC+6", value = "6")]
    P6 = 6,
    #[option(name = "UTC+7", value = "7")]
    P7 = 7,
    #[option(name = "UTC+8", value = "8")]
    P8 = 8,
    #[option(name = "UTC+9", value = "9")]
    P9 = 9,
    #[option(name = "UTC+10", value = "10")]
    P10 = 10,
    #[option(name = "UTC+11", value = "11")]
    P11 = 11,
    #[option(name = "UTC+12", value = "12")]
    P12 = 12,
}

impl From<GraphTopTimezone> for UtcOffset {
    fn from(tz: GraphTopTimezone) -> Self {
        let seconds = match tz {
            GraphTopTimezone::M12 => -12 * 3600,
            GraphTopTimezone::M11 => -11 * 3600,
            GraphTopTimezone::M10 => -10 * 3600,
            GraphTopTimezone::M9 => -9 * 3600,
            GraphTopTimezone::M8 => -8 * 3600,
            GraphTopTimezone::M7 => -7 * 3600,
            GraphTopTimezone::M6 => -6 * 3600,
            GraphTopTimezone::M5 => -5 * 3600,
            GraphTopTimezone::M4 => -4 * 3600,
            GraphTopTimezone::M3 => -3 * 3600,
            GraphTopTimezone::M2 => -2 * 3600,
            #[allow(clippy::neg_multiply)]
            GraphTopTimezone::M1 => -1 * 3600,
            #[allow(clippy::erasing_op)]
            GraphTopTimezone::P0 => 0 * 3600,
            #[allow(clippy::identity_op)]
            GraphTopTimezone::P1 => 1 * 3600,
            GraphTopTimezone::P2 => 2 * 3600,
            GraphTopTimezone::P3 => 3 * 3600,
            GraphTopTimezone::P4 => 4 * 3600,
            GraphTopTimezone::P5 => 5 * 3600,
            GraphTopTimezone::P6 => 6 * 3600,
            GraphTopTimezone::P7 => 7 * 3600,
            GraphTopTimezone::P8 => 8 * 3600,
            GraphTopTimezone::P9 => 9 * 3600,
            GraphTopTimezone::P10 => 10 * 3600,
            GraphTopTimezone::P11 => 11 * 3600,
            GraphTopTimezone::P12 => 12 * 3600,
        };

        Self::from_whole_seconds(seconds).unwrap()
    }
}

async fn slash_graph(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Graph::from_interaction(command.input_data())?;

    graph(ctx, (&mut command).into(), args).await
}

// Takes a `CommandOrigin` since `require_link` does not take `InteractionCommand`
async fn graph(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Graph) -> Result<()> {
    let tuple_option = match args {
        Graph::Medals(args) => {
            let name = match username!(ctx, orig, args) {
                Some(name) => name,
                None => match ctx.psql().get_user_osu(orig.user_id()?).await {
                    Ok(Some(osu)) => osu.into_username(),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.wrap_err("failed to get username"));
                    }
                },
            };

            medals_graph(&ctx, &orig, &name)
                .await
                .wrap_err("failed to create medals graph")?
        }
        Graph::PlaycountReplays(args) => {
            let name = match username!(ctx, orig, args) {
                Some(name) => name,
                None => match ctx.psql().get_user_osu(orig.user_id()?).await {
                    Ok(Some(osu)) => osu.into_username(),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.wrap_err("failed to get username"));
                    }
                },
            };

            let mut flags = ProfileGraphFlags::all();

            if let Some(ShowHideOption::Hide) = args.playcount {
                flags &= !ProfileGraphFlags::PLAYCOUNT;
            }

            if let Some(ShowHideOption::Hide) = args.replays {
                flags &= !ProfileGraphFlags::REPLAYS;
            }

            if let Some(ShowHideOption::Hide) = args.badges {
                flags &= !ProfileGraphFlags::BADGES;
            }

            if flags.is_empty() {
                return orig.error(&ctx, ":clown:").await;
            }

            playcount_replays_graph(&ctx, &orig, &name, flags)
                .await
                .wrap_err("failed to create profile graph")?
        }
        Graph::Rank(args) => {
            let (name, mode) = name_mode!(ctx, orig, args);
            let user_args = UserArgs::new(name.as_str(), mode);

            rank_graph(&ctx, &orig, &name, &user_args)
                .await
                .wrap_err("failed to create rank graph")?
        }
        Graph::Sniped(args) => {
            let name = match username!(ctx, orig, args) {
                Some(name) => name,
                None => match ctx.psql().get_user_osu(orig.user_id()?).await {
                    Ok(Some(osu)) => osu.into_username(),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.wrap_err("failed to get username"));
                    }
                },
            };

            sniped_graph(&ctx, &orig, &name)
                .await
                .wrap_err("failed to create snipe graph")?
        }
        Graph::SnipeCount(args) => {
            let name = match username!(ctx, orig, args) {
                Some(name) => name,
                None => match ctx.psql().get_user_osu(orig.user_id()?).await {
                    Ok(Some(osu)) => osu.into_username(),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.wrap_err("failed to get username"));
                    }
                },
            };

            snipe_count_graph(&ctx, &orig, &name)
                .await
                .wrap_err("failed to create snipe count graph")?
        }
        Graph::Top(args) => {
            let (name, mode) = name_mode!(ctx, orig, args);
            let user_args = UserArgs::new(name.as_str(), mode);
            let tz = args.timezone.map(UtcOffset::from);

            top_graph(&ctx, &orig, &name, user_args, args.order, tz)
                .await
                .wrap_err("failed to create top graph")?
        }
    };

    let (user, graph) = match tuple_option {
        Some(tuple) => tuple,
        None => return Ok(()),
    };

    let embed = GraphEmbed::new(&user).build();

    let builder = MessageBuilder::new()
        .embed(embed)
        .attachment("graph.png", graph);

    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 711;
const LEN: usize = (W * H) as usize;

async fn medals_graph(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
) -> Result<Option<(User, Vec<u8>)>> {
    let user_args = UserArgs::new(name, GameMode::Osu);

    let mut user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    if let Some(ref mut medals) = user.medals {
        medals.sort_unstable_by_key(|medal| medal.achieved_at);
    }

    let bytes = match super::medals::stats::graph(user.medals.as_ref().unwrap(), W, H) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` does not have any medals");
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(ctx, &builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", err.wrap_err("Failed to create medals graph"));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn playcount_replays_graph(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
    flags: ProfileGraphFlags,
) -> Result<Option<(User, Vec<u8>)>> {
    let user_args = UserArgs::new(name, GameMode::Osu);

    let mut user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let params = ProfileGraphParams::new(ctx, &mut user)
        .width(W)
        .height(H)
        .flags(flags);

    let bytes = match profile::graphs(params).await {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` does not have enough playcount data points");
            let builder = &MessageBuilder::new().embed(content);
            orig.create_message(ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", err.wrap_err("Failed to create profile graph"));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn rank_graph(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> Result<Option<(User, Vec<u8>)>> {
    let user = match get_user(ctx, user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    fn draw_graph(user: &User) -> Result<Option<Vec<u8>>> {
        let mut buf = vec![0; LEN * 3];

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
            root.fill(&background)
                .wrap_err("failed to fill background")?;

            let style: fn(RGBColor) -> ShapeStyle = |color| ShapeStyle {
                color: color.to_rgba(),
                filled: false,
                stroke_width: 1,
            };

            let mut chart = ChartBuilder::on(&root)
                .x_label_area_size(40)
                .y_label_area_size(y_label_area_size)
                .margin(10)
                .margin_left(6)
                .build_cartesian_2d(0_u32..history_len.saturating_sub(1) as u32, min..max)
                .wrap_err("failed to build chart")?;

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
                .draw()
                .wrap_err("failed to draw mesh")?;

            let data = (0..).zip(history.iter().map(|rank| -(*rank as i32)));

            let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
            let border_style = style(RGBColor(0, 208, 138)).stroke_width(3);
            let series = AreaSeries::new(data, min, area_style).border_style(border_style);
            chart.draw_series(series).wrap_err("failed to draw area")?;

            let max_coords = (min_idx as u32, max);
            let circle = Circle::new(max_coords, 9_u32, style(GREEN));

            chart
                .draw_series(std::iter::once(circle))
                .wrap_err("failed to draw max circle")?
                .label(format!("Peak: #{}", with_comma_int(-max)))
                .legend(|(x, y)| Circle::new((x, y), 5_u32, style(GREEN)));

            let min_coords = (max_idx as u32, min);
            let circle = Circle::new(min_coords, 9_u32, style(RED));

            chart
                .draw_series(std::iter::once(circle))
                .wrap_err("failed to draw min circle")?
                .label(format!("Worst: #{}", with_comma_int(-min)))
                .legend(|(x, y)| Circle::new((x, y), 5_u32, style(RED)));

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
                .draw()
                .wrap_err("failed to draw legend")?;
        }

        // Encode buf to png
        let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
        let png_encoder = PngEncoder::new(&mut png_bytes);

        png_encoder
            .write_image(&buf, W, H, ColorType::Rgb8)
            .wrap_err("failed to encode image")?;

        Ok(Some(png_bytes))
    }

    let bytes = match draw_graph(&user) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` has no available rank data :(");
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", err.wrap_err("Failed to draw rank graph"));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn sniped_graph(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
) -> Result<Option<(User, Vec<u8>)>> {
    let user_args = UserArgs::new(name, GameMode::Osu);

    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let (sniper, snipee) = if ctx.contains_country(user.country_code.as_str()) {
        let now = OffsetDateTime::now_utc();
        let sniper_fut =
            ctx.client()
                .get_national_snipes(&user, true, now - Duration::weeks(8), now);
        let snipee_fut =
            ctx.client()
                .get_national_snipes(&user, false, now - Duration::weeks(8), now);

        match tokio::try_join!(sniper_fut, snipee_fut) {
            Ok((mut sniper, snipee)) => {
                sniper.retain(|score| score.sniped.is_some());

                (sniper, snipee)
            }
            Err(err) => {
                let _ = orig.error(ctx, HUISMETBENEN_ISSUE).await;

                return Err(err.wrap_err("failed to get sniper or snipee"));
            }
        }
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        orig.error(ctx, content).await?;

        return Ok(None);
    };

    let bytes = match sniped::graphs(user.username.as_str(), &sniper, &snipee, W, H) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content =
                format!("`{name}` was neither sniped nor sniped other people in the last 8 weeks");
            let builder = MessageBuilder::new().embed(content);
            orig.create_message(ctx, &builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", err.wrap_err("Failed to create sniped graph"));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn snipe_count_graph(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
) -> Result<Option<(User, Vec<u8>)>> {
    let user_args = UserArgs::new(name, GameMode::Osu);

    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let player = if ctx.contains_country(user.country_code.as_str()) {
        let player_fut = ctx
            .client()
            .get_snipe_player(&user.country_code, user.user_id);

        match player_fut.await {
            Ok(counts) => counts,
            Err(err) => {
                warn!("{:?}", err.wrap_err("Failed to get snipe player"));
                let content = format!("`{name}` has never had any national #1s");
                let builder = MessageBuilder::new().embed(content);
                orig.create_message(ctx, &builder).await?;

                return Ok(None);
            }
        }
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        orig.error(ctx, content).await?;

        return Ok(None);
    };

    let graph_result =
        player_snipe_stats::graphs(&player.count_first_history, &player.count_sr_spread, W, H);

    let bytes = match graph_result {
        Ok(graph) => graph,
        Err(err) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", err.wrap_err("Failed to create snipe count graph"));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn top_graph(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
    user_args: UserArgs<'_>,
    order: GraphTopOrder,
    tz: Option<UtcOffset>,
) -> Result<Option<(User, Vec<u8>)>> {
    let mode = user_args.mode;
    let score_args = ScoreArgs::top(100);

    let (user, mut scores) = match get_user_and_scores(ctx, user_args, &score_args).await {
        Ok(tuple) => tuple,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            orig.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user or scores");

            return Err(report);
        }
    };

    if scores.is_empty() {
        let content = "User's top scores are empty";
        orig.error(ctx, content).await?;

        return Ok(None);
    }

    let caption = format!(
        "{name}'{genitive} top {mode}scores",
        name = user.username,
        genitive = if user.username.ends_with('s') {
            ""
        } else {
            "s"
        },
        mode = match mode {
            GameMode::Osu => "",
            GameMode::Taiko => "taiko ",
            GameMode::Catch => "ctb ",
            GameMode::Mania => "mania ",
        }
    );

    let tz = tz.unwrap_or_else(|| CountryCode::from(user.country_code.clone()).timezone());

    let graph_result = match order {
        GraphTopOrder::Date => top_graph_date(caption, &mut scores)
            .await
            .wrap_err("Failed to create top date graph"),
        GraphTopOrder::Index => top_graph_index(caption, &scores)
            .await
            .wrap_err("Failed to create top index graph"),
        GraphTopOrder::Time => top_graph_time(caption, &mut scores, tz)
            .await
            .wrap_err("Failed to create top time graph"),
    };

    let bytes = match graph_result {
        Ok(graph) => graph,
        Err(err) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;
            warn!("{err:?}");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn top_graph_date(caption: String, scores: &mut [Score]) -> Result<Vec<u8>> {
    let max = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scores.last().and_then(|s| s.pp).unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    scores.sort_unstable_by_key(|s| s.ended_at);
    let dates: Vec<_> = scores.iter().map(|s| s.ended_at).collect();

    let first = dates[0];
    let last = dates[dates.len() - 1];

    let len = (W * H) as usize;
    let mut buf = vec![0; len * 3];

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let caption_style = ("sans-serif", 25_i32, FontStyle::Bold, &WHITE);

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(40_i32)
            .y_label_area_size(60_i32)
            .margin_top(5_i32)
            .margin_right(15_i32)
            .caption(caption, caption_style)
            .build_cartesian_2d(Monthly(first..last), min_adj..max_adj)
            .wrap_err("failed to build chart")?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .y_label_formatter(&|pp| format!("{pp:.0}pp"))
            .x_label_formatter(&|datetime| datetime.date().to_string())
            .label_style(("sans-serif", 16_i32, &WHITE))
            .bold_line_style(&WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw mesh")?;

        let point_style = RGBColor(2, 186, 213).mix(0.7).filled();
        // let border_style = RGBColor(30, 248, 178).mix(0.9).filled();
        let border_style = WHITE.mix(0.9).stroke_width(1);

        let iter = scores.iter().filter_map(|s| Some((s.ended_at, s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, point_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw main points")?
            .label(format!("Max: {max}pp"))
            .legend(EmptyElement::at);

        let iter = scores.iter().filter_map(|s| Some((s.ended_at, s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, border_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw point borders")?
            .label(format!("Min: {min}pp"))
            .legend(EmptyElement::at);

        chart
            .configure_series_labels()
            .border_style(WHITE.mix(0.6).stroke_width(1))
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::MiddleLeft)
            .legend_area_size(0_i32)
            .label_font(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw legend")?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.write_image(&buf, W, H, ColorType::Rgb8)?;

    Ok(png_bytes)
}

async fn top_graph_index(caption: String, scores: &[Score]) -> Result<Vec<u8>> {
    let max = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scores.last().and_then(|s| s.pp).unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    let len = (W * H) as usize;
    let mut buf = vec![0; len * 3];

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let caption_style = ("sans-serif", 25_i32, FontStyle::Bold, &WHITE);

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(40_i32)
            .y_label_area_size(60_i32)
            .margin_top(5_i32)
            .margin_right(15_i32)
            .caption(caption, caption_style)
            .build_cartesian_2d(1..scores.len(), min_adj..max_adj)
            .wrap_err("failed to build chart")?;

        chart
            .configure_mesh()
            .y_label_formatter(&|pp| format!("{pp:.0}pp"))
            .label_style(("sans-serif", 16_i32, &WHITE))
            .bold_line_style(&WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw mesh")?;

        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = RGBColor(0, 208, 138).stroke_width(3);
        let iter = (1..).zip(scores).filter_map(|(i, s)| Some((i, s.pp?)));
        let series = AreaSeries::new(iter, 0.0, area_style).border_style(border_style);

        chart
            .draw_series(series)
            .wrap_err("failed to draw area")?
            .label(format!("Max: {max}pp"))
            .legend(EmptyElement::at);

        // TODO: remove?
        let iter = (1..)
            .zip(scores)
            .filter_map(|(i, s)| Some((i, s.pp?)))
            .take(0);

        let series = AreaSeries::new(iter, 0.0, &WHITE).border_style(&WHITE);

        chart
            .draw_series(series)
            .wrap_err("failed to draw second area")?
            .label(format!("Min: {min}pp"))
            .legend(EmptyElement::at);

        chart
            .configure_series_labels()
            .border_style(WHITE.mix(0.6).stroke_width(1))
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::UpperRight)
            .legend_area_size(0_i32)
            .label_font(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw legend")?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);

    png_encoder
        .write_image(&buf, W, H, ColorType::Rgb8)
        .wrap_err("failed to encode image")?;

    Ok(png_bytes)
}

async fn top_graph_time(
    mut caption: String,
    scores: &mut [Score],
    tz: UtcOffset,
) -> Result<Vec<u8>> {
    fn date_to_value(date: OffsetDateTime) -> u32 {
        date.hour() as u32 * 60 + date.minute() as u32
    }

    let _ = write!(caption, " (UTC{tz})");

    let mut hours = [0_u32; 24];

    let max = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
    let max_adj = max + 5.0;

    let min = scores.last().and_then(|s| s.pp).unwrap_or(0.0);
    let min_adj = (min - 5.0).max(0.0);

    for score in scores.iter_mut() {
        score.ended_at = score.ended_at.to_offset(tz);
        hours[score.ended_at.hour() as usize] += 1;
    }

    scores.sort_unstable_by_key(|s| s.ended_at.time());

    let max_hours = hours.iter().max().copied().unwrap_or(0);

    let len = (W * H) as usize;
    let mut buf = vec![0; len * 3];

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let caption_style = ("sans-serif", 25_i32, FontStyle::Bold, &WHITE);

        let x_label_area_size = 50;
        let y_label_area_size = 60;
        let right_y_label_area_size = 45;
        let margin_bottom = 5;
        let margin_top = 5;
        let margin_right = 15;

        // Draw bars
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .right_y_label_area_size(right_y_label_area_size)
            .margin_bottom(margin_bottom)
            .margin_top(margin_top)
            .margin_right(margin_right)
            .caption(caption, caption_style)
            .build_cartesian_2d((0_u32..23_u32).into_segmented(), 0_u32..max_hours)
            .wrap_err("failed to build bar chart")?
            .set_secondary_coord((0_u32..23_u32).into_segmented(), 0_u32..max_hours);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .disable_y_axis()
            .x_labels(24)
            .x_desc("Hour of the day")
            .label_style(("sans-serif", 16_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw primary bar mesh")?;

        chart
            .configure_secondary_axes()
            .y_desc("#  of  plays  set")
            .label_style(("sans-serif", 16_i32, &WHITE))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw secondary mesh")?;

        let counts = ScoreHourCounts::new(hours);
        chart
            .draw_secondary_series(counts)
            .wrap_err("failed to draw bars")?;

        // Draw points
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .right_y_label_area_size(right_y_label_area_size)
            .margin_bottom(margin_bottom)
            .margin_top(margin_top)
            .margin_right(margin_right)
            .caption("", caption_style)
            .build_cartesian_2d(0_u32..24 * 60, min_adj..max_adj)
            .wrap_err("failed to build point chart")?
            .set_secondary_coord(0_u32..24 * 60, min_adj..max_adj);

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_x_axis()
            .y_label_formatter(&|pp| format!("{pp:.0}pp"))
            .x_label_formatter(&|value| format!("{}:{:0>2}", value / 60, value % 60))
            .label_style(("sans-serif", 16_i32, &WHITE))
            .bold_line_style(&WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw point mesh")?;

        // Draw secondary axis just to hide its values so that
        // the left hand values aren't displayed instead
        chart
            .configure_secondary_axes()
            .label_style(("", 16_i32, &WHITE.mix(0.0)))
            .axis_style(WHITE.mix(0.0))
            .draw()
            .wrap_err("failed to draw secondary points")?;

        let point_style = RGBColor(2, 186, 213).mix(0.7).filled();
        let border_style = WHITE.mix(0.9).stroke_width(1);

        let iter = scores
            .iter()
            .filter_map(|s| Some((date_to_value(s.ended_at), s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, point_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw primary points")?
            .label(format!("Max: {max}pp"))
            .legend(EmptyElement::at);

        let iter = scores
            .iter()
            .filter_map(|s| Some((date_to_value(s.ended_at), s.pp?)));

        let series = PointSeries::of_element(iter, 3_i32, border_style, &|coord, size, style| {
            EmptyElement::at(coord) + Circle::new((0, 0), size, style)
        });

        chart
            .draw_series(series)
            .wrap_err("failed to draw primary points borders")?
            .label(format!("Min: {min}pp"))
            .legend(EmptyElement::at);

        chart
            .configure_series_labels()
            .border_style(WHITE.mix(0.6).stroke_width(1))
            .background_style(RGBColor(7, 23, 17))
            .position(SeriesLabelPosition::Coordinate((W as f32 / 4.5) as i32, 10))
            .legend_area_size(0_i32)
            .label_font(("sans-serif", 16_i32, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw legend")?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(len);
    let png_encoder = PngEncoder::new(&mut png_bytes);

    png_encoder
        .write_image(&buf, W, H, ColorType::Rgb8)
        .wrap_err("failed to encode image")?;

    Ok(png_bytes)
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

        let mix = if count > 0 { 0.5 } else { 0.0 };
        let style = RGBColor(0, 126, 153).mix(mix).filled();

        let mut rect = Rectangle::new([top_left, bot_right], style);
        rect.set_margin(0, 1, 2, 2);

        Some(rect)
    }
}

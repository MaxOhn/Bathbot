use std::collections::BTreeMap;

use bathbot_macros::command;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher, MessageBuilder,
};
use eyre::{ContextCompat, Report, Result, WrapErr};
use plotters::prelude::*;
use plotters_skia::SkiaBackend;
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use skia_safe::{surfaces, EncodedImageFormat};
use time::Date;
use twilight_model::guild::Permissions;

use super::{SnipeGameMode, SnipePlayerStats};
use crate::{
    commands::osu::require_link,
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, PlayerSnipeStatsEmbed},
    manager::redis::{osu::{UserArgs, UserArgsError}, },
    util::Monthly,
    Context,
};

#[command]
#[desc("Stats about a user's #1 scores in their country leaderboards")]
#[help(
    "Stats about a user's #1 scores in their country leaderboards.\n\
    Data for osu!standard originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("pss")]
#[group(Osu)]
async fn prefix_playersnipestats(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipePlayerStats::args(args, None);

    player_stats(CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Stats about a user's #1 ctb scores in their country leaderboards")]
#[help(
    "Stats about a user's #1 ctb scores in their country leaderboards.\n\
    Data for osu!catch originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("pssc", "playersnipestatscatch")]
#[group(Catch)]
async fn prefix_playersnipestatsctb(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipePlayerStats::args(args, Some(GameMode::Catch));

    player_stats(CommandOrigin::from_msg(msg, permissions), args).await
}

#[command]
#[desc("Stats about a user's #1 mania scores in their country leaderboards")]
#[help(
    "Stats about a user's #1 mania scores in their country leaderboards.\n\
    Data for osu!mania originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[alias("pssm")]
#[group(Mania)]
async fn prefix_playersnipestatsmania(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = SnipePlayerStats::args(args, Some(GameMode::Mania));

    player_stats(CommandOrigin::from_msg(msg, permissions), args).await
}

pub(super) async fn player_stats(
    orig: CommandOrigin<'_>,
    args: SnipePlayerStats<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let legacy_scores = match config.score_data {
        Some(score_data) => score_data.is_legacy(),
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| {
                    config.score_data.map(ScoreData::is_legacy)
                })
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let report = Report::new(err).wrap_err("Failed to get user");

            return Err(report);
        }
    };


    let country_code = user.country_code.as_str();
    let username = user.username.as_str();
    let user_id = user.user_id.to_native();

    let client = Context::client();

    let player_fut = if Context::huismetbenen()
        .is_supported(country_code, mode)
        .await
    {
        client.get_snipe_player(country_code, user_id, mode)
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        return orig.error(content).await;
    };

    let history_fut = client.get_snipe_player_history(country_code, user_id, mode);

    let (player, history) = match tokio::try_join!(player_fut, history_fut) {
        Ok((Some(player), history)) => (player, history),
        Ok((None, _)) => {
            let content = format!(
                "`{username}` does not have any national #1s in {mode}",
                mode = match mode {
                    GameMode::Osu => "osu!standard",
                    GameMode::Taiko => "osu!taiko",
                    GameMode::Catch => "osu!catch",
                    GameMode::Mania => "osu!mania",
                }
            );

            let builder = MessageBuilder::new().embed(content);
            orig.create_message(builder).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let graph = match graphs(&history, &player.count_sr_spread, W, H) {
        Ok(graph) => Some(graph),
        Err(err) => {
            warn!(?err, "Failed to create graph");

            None
        }
    };

    let oldest = if let Some(map_id) = player.oldest_map_id {
        let score_fut =
            Context::osu_scores().user_on_map_single(user_id, map_id, mode, None, legacy_scores);

        let map_fut = Context::osu_map().map(map_id, None);

        match tokio::join!(score_fut, map_fut) {
            (Ok(score), Ok(map)) => Some((score.score, map)),
            (Err(err), _) => {
                let _ = orig.error(OSU_API_ISSUE).await;

                return Err(Report::new(err).wrap_err("Failed to get oldest score"));
            }
            (_, Err(err)) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(Report::new(err).wrap_err("Failed to get map of oldest score"));
            }
        }
    } else {
        None
    };

    let embed = PlayerSnipeStatsEmbed::new(&user, player, oldest.as_ref())
        .await
        .build();

    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.attachment("stats_graph.png", bytes);
    }

    orig.create_message(builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 350;

pub fn graphs(
    history: &BTreeMap<Date, u32>,
    stars: &BTreeMap<i8, u32>,
    w: u32,
    h: u32,
) -> Result<Vec<u8>> {
    let mut surface =
        surfaces::raster_n32_premul((w as i32, h as i32)).wrap_err("Failed to create surface")?;

    let style: fn(RGBColor) -> ShapeStyle = |color| ShapeStyle {
        color: color.to_rgba(),
        filled: false,
        stroke_width: 1,
    };

    {
        let root = SkiaBackend::new(surface.canvas(), w, h).into_drawing_area();

        let background = RGBColor(19, 43, 33);
        root.fill(&background)
            .wrap_err("failed to fill background")?;

        let star_canvas = if history.len() > 1 {
            let (left, right) = root.split_horizontally(3 * w / 5);

            let (min, max) = history
                .iter()
                .map(|(_, n)| *n)
                .fold((u32::MAX, 0), |(min, max), curr| {
                    (min.min(curr), max.max(curr))
                });

            let min = match min < 20 {
                true => 0,
                false => min - min / 11,
            };

            let first = *history.keys().next().unwrap();
            let last = *history.keys().last().unwrap();

            let mut chart = ChartBuilder::on(&left)
                .margin(9)
                .caption("National #1 Count History", ("sans-serif", 30, &WHITE))
                .x_label_area_size(20)
                .y_label_area_size(40)
                .build_cartesian_2d(Monthly(first..last), min..max + 1)
                .wrap_err("failed to build left chart")?;

            // Mesh and labels
            chart
                .configure_mesh()
                .disable_x_mesh()
                .x_labels(8)
                .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month() as u8))
                .label_style(("sans-serif", 15, &WHITE))
                .bold_line_style(WHITE.mix(0.3))
                .axis_style(RGBColor(7, 18, 14))
                .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
                .draw()
                .wrap_err("failed to draw left mesh")?;

            // Draw area
            let iter = history.iter().map(|(date, n)| (*date, *n));
            let area_style = RGBColor(2, 186, 213).mix(0.7).filled();
            let border_style = style(RGBColor(0, 208, 138)).stroke_width(3);
            let series = AreaSeries::new(iter, 0, area_style).border_style(border_style);
            chart
                .draw_series(series)
                .wrap_err("failed to draw left series")?;

            right
        } else {
            root
        };

        // Star spread graph
        let max = stars
            .iter()
            .filter(|(sr, _)| **sr >= 0)
            .map(|(_, n)| n)
            .fold(0, |max, &curr| max.max(curr));

        let first = stars.keys().copied().find(|sr| *sr >= 0).unwrap_or(0) as u32;
        let last = stars
            .keys()
            .copied()
            .filter(|sr| *sr >= 0)
            .last()
            .unwrap_or(0) as u32;

        let mut chart = ChartBuilder::on(&star_canvas)
            .x_label_area_size(30)
            .y_label_area_size(40)
            .margin_right(15)
            .caption("Star rating spread", ("sans-serif", 30, &WHITE))
            .build_cartesian_2d((first..last).into_segmented(), 0..max + 1)
            .wrap_err("failed to build right chart")?;

        // Mesh and labels
        chart
            .configure_mesh()
            .disable_x_mesh()
            .x_labels(15)
            .label_style(("sans-serif", 15, &WHITE))
            .bold_line_style(WHITE.mix(0.3))
            .axis_style(RGBColor(7, 18, 14))
            .axis_desc_style(("sans-serif", 16, FontStyle::Bold, &WHITE))
            .draw()
            .wrap_err("failed to draw right mesh")?;

        // Histogram bars
        let area_style = RGBColor(2, 186, 213).mix(0.7).filled();

        let iter = stars
            .iter()
            .filter(|(sr, _)| **sr >= 0)
            .map(|(stars, n)| (*stars as u32, *n));

        let series = Histogram::vertical(&chart)
            .style(area_style)
            .data(iter)
            .margin(3);

        chart
            .draw_series(series)
            .wrap_err("failed to draw right series")?;
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

impl<'m> SnipePlayerStats<'m> {
    fn args(mut args: Args<'m>, mode: Option<GameMode>) -> Self {
        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Self {
            mode: mode.and_then(SnipeGameMode::try_from_mode),
            name,
            discord,
        }
    }
}

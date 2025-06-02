use std::fmt::Write;

use bathbot_macros::command;
use bathbot_model::RelaxPlayersDataResponse;
use bathbot_util::{
    EmbedBuilder, FooterBuilder, MessageBuilder, MessageOrigin, attachment,
    constants::{GENERAL_ISSUE, RELAX_ICON_URL},
    datetime::NAIVE_DATETIME_FORMAT,
    fields, matcher,
    numbers::WithComma,
};
use eyre::{Context as _, ContextCompat, Report, Result};
use plotters::{
    chart::{ChartBuilder, ChartContext},
    coord::{Shift, types::RangedCoordi32},
    prelude::{Cartesian2d, Circle, DrawingArea, IntoDrawingArea, PathElement},
    series::AreaSeries,
    style::{BLACK, Color, RGBColor, WHITE},
};
use plotters_backend::FontStyle;
use plotters_skia::SkiaBackend;
use rosu_v2::{
    error::OsuError,
    model::{GameMode, Grade},
    prelude::MonthlyCount,
    request::UserId,
};
use skia_safe::{EncodedImageFormat, Surface, surfaces};
use time::Date;
use twilight_model::{
    guild::Permissions,
    id::{Id, marker::UserMarker},
};

use crate::{
    commands::osu::{
        relax::{RX_PROFILE_DESC, RX_PROFILE_HELP, RelaxProfile, relax_author_builder},
        require_link,
    },
    core::{
        Context,
        commands::{CommandOrigin, prefix::Args},
    },
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::{Monthly, osu::grade_emote},
};

impl<'a> RelaxProfile<'a> {
    fn args(args: Args<'a>) -> Self {
        let mut name = None;
        let mut discord = None;

        for arg in args {
            if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self { name, discord }
    }
}

#[command]
#[desc(RX_PROFILE_DESC)]
#[help(RX_PROFILE_HELP)]
#[usage("[username]")]
#[example("chiffa")]
#[alias("relaxp", "rxprofile", "rxp")]
#[group(Osu)]
async fn prefix_relaxprofile(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = RelaxProfile::args(args);
    let orig = CommandOrigin::from_msg(msg, permissions);

    relax_profile(orig, args).await
}

pub(super) async fn relax_profile(orig: CommandOrigin<'_>, args: RelaxProfile<'_>) -> Result<()> {
    let msg_owner = orig.user_id()?;
    let config = match Context::user_config().with_osu_id(msg_owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

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
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let user_id = user.user_id.to_native();
    let client = Context::client();
    let info_fut = client.get_relax_player(user_id);

    let guild = orig.guild_id();
    let user_id_fut = Context::user_config().discord_from_osu_id(user_id);

    let (info_res, user_id_res) = tokio::join!(info_fut, user_id_fut);

    let discord_id = match user_id_res {
        Ok(user) => match (guild, user) {
            (Some(guild), Some(user)) => Context::cache()
                .member(guild, user) // make sure the user is in the guild
                .await?
                .map(|_| user),
            _ => None,
        },
        Err(err) => {
            warn!(?err, "Failed to get discord id from osu user id");

            None
        }
    };

    let info_res = match info_res {
        Ok(Some(info_res)) => info_res,
        Ok(None) => {
            return orig
                .error(format!("Relax player `{}` not found", user.username))
                .await;
        }
        Err(err) => {
            warn!(?err, "Failed to fetch user relax player info");

            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());
    let pagination = RelaxProfileArgs::new(user, discord_id, info_res, origin);

    let graph = match relax_playcount_graph(&pagination) {
        Ok(graph) => graph,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to draw a relax playcount graph"));
        }
    };

    let builder = MessageBuilder::new()
        .embed(relax_profile_builder(pagination).unwrap())
        .attachment("graph.png", graph);

    orig.create_message(builder).await?;

    Ok(())
}

pub struct RelaxProfileArgs {
    user: CachedUser,
    discord_id: Option<Id<UserMarker>>,
    info: RelaxPlayersDataResponse,
    origin: MessageOrigin,
}

impl RelaxProfileArgs {
    pub fn new(
        user: CachedUser,
        discord_id: Option<Id<UserMarker>>,
        info: RelaxPlayersDataResponse,
        origin: MessageOrigin,
    ) -> Self {
        Self {
            user,
            discord_id,
            info,
            origin,
        }
    }
}

pub fn relax_profile_builder(args: RelaxProfileArgs) -> Result<EmbedBuilder> {
    let stats = &args.info;
    let mut description = "__**Relax user statistics".to_string();
    if let Some(discord_id) = args.discord_id {
        let _ = write!(description, " for <@{discord_id}>");
    };

    description.push_str(":**__");
    let _ = writeln!(
        description,
        "\n
        Accuracy: [`{acc:.2}%`]({origin} \"{acc}\") • \
        Playcount: `{playcount}`",
        origin = args.origin,
        acc = stats.total_accuracy.unwrap_or_default(),
        playcount = WithComma::new(stats.playcount)
    );
    let ss_grades = format!("{}{}", stats.count_ss, grade_emote(Grade::X));
    let s_grades = format!("{}{}", stats.count_s, grade_emote(Grade::S));
    let a_grades = format!("{}{}", stats.count_a, grade_emote(Grade::A));
    let fields = fields![
        "Count SS", ss_grades, true;
        "Count S", s_grades, true;
        "Count A", a_grades, true;
    ];
    let embed = EmbedBuilder::new()
        .author(relax_author_builder(&args.user, &args.info))
        .description(description)
        .fields(fields)
        .image(attachment("graph.png"))
        .thumbnail(args.user.avatar_url.as_ref())
        .footer(relax_footer_builder(&args));

    Ok(embed)
}

fn relax_footer_builder(args: &RelaxProfileArgs) -> FooterBuilder {
    let last_update = format!(
        "Last update: {}",
        args.info
            .updated_at
            .unwrap()
            .format(NAIVE_DATETIME_FORMAT)
            .unwrap()
    );

    FooterBuilder::new(last_update).icon_url(RELAX_ICON_URL)
}

// FIXME: This is a mess. @chiffa move an existing graph into bathbot-utils and
// use that or something
const W: u32 = 590;
const H: u32 = 170;
fn relax_playcount_graph(args: &RelaxProfileArgs) -> Result<Vec<u8>> {
    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;
    let root = create_root(&mut surface, W, H)?;
    let playcounts: Vec<MonthlyCount> = args
        .info
        .playcounts_per_month
        .iter()
        .map(|playcount| MonthlyCount {
            start_date: playcount.date.date(),
            count: playcount.playcount as i32,
        })
        .collect();
    draw_playcounts(&playcounts, &root)?;
    let canvas: Vec<u8> = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();
    Ok(canvas)
}
const PLAYCOUNTS_AREA_COLOR: RGBColor = RGBColor(0, 116, 193);
const PLAYCOUNTS_BORDER_COLOR: RGBColor = RGBColor(102, 174, 222);
type Area<'b> = DrawingArea<SkiaBackend<'b>, Shift>;
type Chart<'a, 'b> = ChartContext<'a, SkiaBackend<'b>, Cartesian2d<Monthly<Date>, RangedCoordi32>>;

fn create_root(surface: &mut Surface, w: u32, h: u32) -> Result<Area<'_>> {
    let root = SkiaBackend::new(surface.canvas(), w, h).into_drawing_area();

    let background = RGBColor(19, 43, 33);
    root.fill(&background)
        .wrap_err("Failed to fill background")?;

    Ok(root)
}
fn draw_playcounts(playcounts: &[MonthlyCount], canvas: &Area<'_>) -> Result<()> {
    let (first, last, max) = first_last_max(playcounts);

    let mut chart = ChartBuilder::on(canvas)
        .margin(12_i32)
        .x_label_area_size(17_i32)
        .y_label_area_size(50_i32)
        .build_cartesian_2d(Monthly(first..last), 0..max)
        .wrap_err("Failed to build playcounts chart")?;

    chart
        .configure_mesh()
        .light_line_style(BLACK.mix(0.0))
        .disable_x_mesh()
        .x_labels(10)
        .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month() as u8))
        .y_desc("Monthly playcount")
        .label_style(("sans-serif", 14_i32, &WHITE))
        .bold_line_style(WHITE.mix(0.3))
        .axis_style(RGBColor(7, 18, 14))
        .axis_desc_style(("sans-serif", 14_i32, FontStyle::Bold, &WHITE))
        .draw()
        .wrap_err("Failed to draw playcounts mesh")?;

    draw_area(
        &mut chart,
        PLAYCOUNTS_AREA_COLOR,
        0.5,
        PLAYCOUNTS_BORDER_COLOR,
        0.6,
        playcounts,
        "Monthly playcount",
    )
    .wrap_err("Failed to draw playcount area")
}
fn draw_area(
    chart: &mut Chart<'_, '_>,
    area_color: RGBColor,
    area_mix: f64,
    border_color: RGBColor,
    border_mix: f64,
    monthly_counts: &[MonthlyCount],
    label: &str,
) -> Result<()> {
    // Draw area
    let iter = monthly_counts
        .iter()
        .map(|MonthlyCount { start_date, count }| (*start_date, *count));

    let series = AreaSeries::new(iter, 0, area_color.mix(area_mix).filled());

    chart
        .draw_series(series.border_style(border_color.stroke_width(1)))
        .wrap_err("Failed to draw area")?
        .label(label)
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 20, y)], area_color.stroke_width(2))
        });

    // Draw circles
    let circles = monthly_counts
        .iter()
        .map(move |MonthlyCount { start_date, count }| {
            let style = border_color.mix(border_mix).filled();

            Circle::new((*start_date, *count), 2_i32, style)
        });

    chart
        .draw_series(circles)
        .wrap_err("Failed to draw circles")?;

    Ok(())
}
fn first_last_max(counts: &[MonthlyCount]) -> (Date, Date, i32) {
    let first = counts.first().unwrap().start_date;
    let last = counts.last().unwrap().start_date;
    let max = counts.iter().map(|c| c.count).max();

    (first, last, max.map_or(2, |m| m.max(2)))
}

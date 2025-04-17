use std::iter;

use bathbot_macros::{HasName, SlashCommand, command};
use bathbot_model::{
    Countries,
    command_fields::{GameModeOption, ShowHideOption, TimezoneOption},
};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{EmbedBuilder, MessageBuilder, constants::GENERAL_ISSUE};
use eyre::{Report, Result, WrapErr};
use image::{DynamicImage, GenericImageView};
use plotters::element::{Drawable, PointCollection};
use plotters_backend::{BackendCoord, DrawingBackend, DrawingErrorKind};
use plotters_skia::SkiaBackend;
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};
use score_rank::score_rank_graph;
use time::UtcOffset;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{Id, marker::UserMarker};

use self::{
    medals::medals_graph,
    playcount_replays::{ProfileGraphFlags, playcount_replays_graph},
    rank::rank_graph,
    snipe_count::snipe_count_graph,
    sniped::sniped_graph,
    top_date::top_graph_date,
    top_index::top_graph_index,
    top_time::top_graph_time,
};
use super::{SnipeGameMode, require_link, user_not_found};
use crate::{
    core::{Context, commands::CommandOrigin},
    embeds::attachment,
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::{CachedUserExt, InteractionCommandExt, interaction::InteractionCommand},
};

mod medals;
mod playcount_replays;
mod rank;
mod score_rank;
mod snipe_count;
mod sniped;
mod top_date;
mod top_index;
mod top_time;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "graph", desc = "Display graphs about some user data")]
pub enum Graph {
    #[command(name = "medals")]
    Medals(GraphMedals),
    #[command(name = "playcount_replays")]
    PlaycountReplays(GraphPlaycountReplays),
    #[command(name = "rank")]
    Rank(GraphRank),
    #[command(name = "score_rank")]
    ScoreRank(GraphScoreRank),
    #[command(name = "sniped")]
    Sniped(GraphSniped),
    #[command(name = "snipe_count")]
    SnipeCount(GraphSnipeCount),
    #[command(name = "top")]
    Top(GraphTop),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "medals", desc = "Display a user's medal progress over time")]
pub struct GraphMedals {
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "playcount_replays",
    desc = "Display a user's playcount and replays watched over time"
)]
pub struct GraphPlaycountReplays {
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(desc = "Specify if the playcount curve should be included")]
    playcount: Option<ShowHideOption>,
    #[command(desc = "Specify if the replay curve should be included")]
    replays: Option<ShowHideOption>,
    #[command(desc = "Specify if the badges should be included")]
    badges: Option<ShowHideOption>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "rank", desc = "Display a user's rank progression over time")]
pub struct GraphRank {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "score_rank",
    desc = "Display a user's score rank progression over time"
)]
pub struct GraphScoreRank {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "sniped", desc = "Display sniped users of the past 8 weeks")]
pub struct GraphSniped {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "snipe_count",
    desc = "Display how a user's national #1 count progressed"
)]
pub struct GraphSnipeCount {
    #[command(desc = "Specify a gamemode")]
    mode: Option<SnipeGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "top",
    desc = "Display a user's top scores pp",
    help = "Display a user's top scores pp.\n\
    The timezone option is only relevant for the `Time` order."
)]
pub struct GraphTop {
    #[command(desc = "Choose by which order the scores should be sorted, defaults to index")]
    order: GraphTopOrder,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(desc = "Specify a timezone (only relevant when ordered by `Time`)")]
    timezone: Option<TimezoneOption>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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

async fn slash_graph(mut command: InteractionCommand) -> Result<()> {
    let args = Graph::from_interaction(command.input_data())?;

    graph((&mut command).into(), args).await
}

// Takes a `CommandOrigin` since `require_link` does not take
// `InteractionCommand`
async fn graph(orig: CommandOrigin<'_>, args: Graph) -> Result<()> {
    let tuple_option = match args {
        Graph::Medals(args) => {
            let user_id = match user_id!(orig, args) {
                Some(user_id) => user_id,
                None => match Context::user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&orig).await,
                    Err(err) => {
                        let _ = orig.error(GENERAL_ISSUE).await;

                        return Err(err);
                    }
                },
            };

            medals_graph(&orig, user_id)
                .await
                .wrap_err("failed to create medals graph")?
        }
        Graph::PlaycountReplays(args) => {
            let user_id = match user_id!(orig, args) {
                Some(user_id) => user_id,
                None => match Context::user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&orig).await,
                    Err(err) => {
                        let _ = orig.error(GENERAL_ISSUE).await;

                        return Err(err);
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
                return orig.error(":clown:").await;
            }

            playcount_replays_graph(&orig, user_id, flags)
                .await
                .wrap_err("failed to create profile graph")?
        }
        Graph::Rank(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);
            let user_args = UserArgs::rosu_id(&user_id, mode).await;

            rank_graph(&orig, user_id, user_args)
                .await
                .wrap_err("Failed to create rank graph")?
        }
        Graph::ScoreRank(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);

            let tuple_option = score_rank_graph(&orig, user_id, mode)
                .await
                .wrap_err("Failed to create score rank graph")?;

            let Some((author, graph)) = tuple_option else {
                return Ok(());
            };

            let embed = EmbedBuilder::new()
                .author(author)
                .image(attachment("graph.png"));

            let builder = MessageBuilder::new()
                .embed(embed)
                .attachment("graph.png", graph);

            orig.create_message(builder).await?;

            return Ok(());
        }
        Graph::Sniped(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);

            sniped_graph(&orig, user_id, mode)
                .await
                .wrap_err("failed to create snipe graph")?
        }
        Graph::SnipeCount(args) => {
            let (user_id, mode) = user_id_mode!(orig, args);

            snipe_count_graph(&orig, user_id, mode)
                .await
                .wrap_err("failed to create snipe count graph")?
        }
        Graph::Top(args) => {
            let owner = orig.user_id()?;

            let config = match Context::user_config().with_osu_id(owner).await {
                Ok(config) => config,
                Err(err) => {
                    let _ = orig.error(GENERAL_ISSUE).await;

                    return Err(err.wrap_err("failed to get user config"));
                }
            };

            let mode = args
                .mode
                .map(GameMode::from)
                .or(config.mode)
                .unwrap_or(GameMode::Osu);

            let (user_id, no_user_specified) = match user_id!(orig, args) {
                Some(user_id) => (user_id, false),
                None => match config.osu {
                    Some(user_id) => (UserId::Id(user_id), true),
                    None => return require_link(&orig).await,
                },
            };

            let user_args = UserArgs::rosu_id(&user_id, mode).await;

            let tz = args
                .timezone
                .map(UtcOffset::from)
                .or_else(|| no_user_specified.then_some(config.timezone).flatten());

            let legacy_scores = match config.score_data {
                Some(score_data) => score_data.is_legacy(),
                None => match orig.guild_id() {
                    Some(guild_id) => Context::guild_config()
                        .peek(guild_id, |config| config.score_data)
                        .await
                        .is_some_and(ScoreData::is_legacy),
                    None => false,
                },
            };

            top_graph(&orig, user_id, user_args, args.order, tz, legacy_scores)
                .await
                .wrap_err("failed to create top graph")?
        }
    };

    let Some((user, graph)) = tuple_option else {
        return Ok(());
    };

    let embed = EmbedBuilder::new()
        .author(user.author_builder(false))
        .image(attachment("graph.png"));

    let builder = MessageBuilder::new()
        .embed(embed)
        .attachment("graph.png", graph);

    orig.create_message(builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 711;

async fn top_graph(
    orig: &CommandOrigin<'_>,
    user_id: UserId,
    user_args: UserArgs,
    order: GraphTopOrder,
    tz: Option<UtcOffset>,
    legacy_scores: bool,
) -> Result<Option<(CachedUser, Vec<u8>)>> {
    let scores_fut = Context::osu_scores()
        .top(200, legacy_scores)
        .exec_with_user(user_args);

    let (user, mut scores) = match scores_fut.await {
        Ok(tuple) => tuple,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;
            orig.error(content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or scores");

            return Err(err);
        }
    };

    if scores.is_empty() {
        let content = "User's top scores are empty";
        orig.error(content).await?;

        return Ok(None);
    }

    let username = user.username.as_str();
    let country_code = user.country_code.as_str();
    let mode = user.mode;

    let caption = format!(
        "{username}'{genitive} top {mode}scores",
        genitive = if username.ends_with('s') { "" } else { "s" },
        mode = match mode {
            GameMode::Osu => "",
            GameMode::Taiko => "taiko ",
            GameMode::Catch => "ctb ",
            GameMode::Mania => "mania ",
        }
    );

    let tz = tz.unwrap_or_else(|| Countries::code(country_code).to_timezone());

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
            let _ = orig.error(GENERAL_ISSUE).await;
            warn!("{err:?}");

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

pub struct BitMapElement<C> {
    img: Vec<u8>,
    size: (u32, u32),
    pos: C,
}

impl<C> BitMapElement<C> {
    /// Be sure the image has been resized beforehand
    pub fn new(img: DynamicImage, pos: C) -> Self {
        let size = img.dimensions();
        let img = img.into_rgba8().into_raw();

        Self { img, size, pos }
    }
}

impl<'a, C> PointCollection<'a, C> for &'a BitMapElement<C> {
    type IntoIter = iter::Once<&'a C>;
    type Point = &'a C;

    #[inline]
    fn point_iter(self) -> Self::IntoIter {
        iter::once(&self.pos)
    }
}

impl<'a, C> Drawable<SkiaBackend<'a>> for BitMapElement<C> {
    #[inline]
    fn draw<I: Iterator<Item = BackendCoord>>(
        &self,
        mut points: I,
        backend: &mut SkiaBackend<'a>,
        _: (u32, u32),
    ) -> Result<(), DrawingErrorKind<<SkiaBackend<'_> as DrawingBackend>::ErrorType>> {
        if let Some((x, y)) = points.next() {
            return backend.blit_bitmap((x, y), self.size, self.img.as_ref());
        }

        Ok(())
    }
}

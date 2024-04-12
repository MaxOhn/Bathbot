use std::{iter, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::{rosu_v2::user::User, Countries};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use image::{DynamicImage, GenericImageView};
use plotters::element::{Drawable, PointCollection};
use plotters_backend::{BackendCoord, DrawingBackend, DrawingErrorKind};
use plotters_skia::SkiaBackend;
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};
use time::UtcOffset;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use self::{
    medals::medals_graph,
    playcount_replays::{playcount_replays_graph, ProfileGraphFlags},
    rank::rank_graph,
    snipe_count::snipe_count_graph,
    sniped::sniped_graph,
    top_date::top_graph_date,
    top_index::top_graph_index,
    top_time::top_graph_time,
};
use super::{require_link, user_not_found};
use crate::{
    commands::{GameModeOption, ShowHideOption, TimezoneOption},
    core::{commands::CommandOrigin, Context, ContextExt},
    embeds::attachment,
    manager::redis::{osu::UserArgs, RedisData},
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

mod medals;
mod playcount_replays;
mod rank;
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
#[command(name = "sniped", desc = "Display sniped users of the past 8 weeks")]
pub struct GraphSniped {
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

async fn slash_graph(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Graph::from_interaction(command.input_data())?;

    graph(ctx, (&mut command).into(), args).await
}

// Takes a `CommandOrigin` since `require_link` does not take
// `InteractionCommand`
async fn graph(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Graph) -> Result<()> {
    let tuple_option = match args {
        Graph::Medals(args) => {
            let user_id = match user_id!(ctx, orig, args) {
                Some(user_id) => user_id,
                None => match ctx.user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err);
                    }
                },
            };

            medals_graph(ctx.cloned(), &orig, user_id)
                .await
                .wrap_err("failed to create medals graph")?
        }
        Graph::PlaycountReplays(args) => {
            let user_id = match user_id!(ctx, orig, args) {
                Some(user_id) => user_id,
                None => match ctx.user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

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
                return orig.error(&ctx, ":clown:").await;
            }

            playcount_replays_graph(ctx.cloned(), &orig, user_id, flags)
                .await
                .wrap_err("failed to create profile graph")?
        }
        Graph::Rank(args) => {
            let (user_id, mode) = user_id_mode!(ctx, orig, args);
            let user_args = UserArgs::rosu_id(ctx.cloned(), &user_id).await.mode(mode);

            rank_graph(ctx.cloned(), &orig, user_id, user_args)
                .await
                .wrap_err("failed to create rank graph")?
        }
        Graph::Sniped(args) => {
            let user_id = match user_id!(ctx, orig, args) {
                Some(user_id) => user_id,
                None => match ctx.user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err);
                    }
                },
            };

            sniped_graph(ctx.cloned(), &orig, user_id)
                .await
                .wrap_err("failed to create snipe graph")?
        }
        Graph::SnipeCount(args) => {
            let user_id = match user_id!(ctx, orig, args) {
                Some(user_id) => user_id,
                None => match ctx.user_config().osu_id(orig.user_id()?).await {
                    Ok(Some(user_id)) => UserId::Id(user_id),
                    Ok(None) => return require_link(&ctx, &orig).await,
                    Err(err) => {
                        let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err);
                    }
                },
            };

            snipe_count_graph(ctx.cloned(), &orig, user_id)
                .await
                .wrap_err("failed to create snipe count graph")?
        }
        Graph::Top(args) => {
            let owner = orig.user_id()?;

            let config = match ctx.user_config().with_osu_id(owner).await {
                Ok(config) => config,
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("failed to get user config"));
                }
            };

            let mode = args
                .mode
                .map(GameMode::from)
                .or(config.mode)
                .unwrap_or(GameMode::Osu);

            let (user_id, no_user_specified) = match user_id!(ctx, orig, args) {
                Some(user_id) => (user_id, false),
                None => match config.osu {
                    Some(user_id) => (UserId::Id(user_id), true),
                    None => return require_link(&ctx, &orig).await,
                },
            };

            let user_args = UserArgs::rosu_id(ctx.cloned(), &user_id).await.mode(mode);

            let tz = args
                .timezone
                .map(UtcOffset::from)
                .or_else(|| no_user_specified.then_some(config.timezone).flatten());

            let legacy_scores = match config.legacy_scores {
                Some(legacy_scores) => legacy_scores,
                None => match orig.guild_id() {
                    Some(guild_id) => ctx
                        .guild_config()
                        .peek(guild_id, |config| config.legacy_scores)
                        .await
                        .unwrap_or(false),
                    None => false,
                },
            };

            top_graph(
                ctx.cloned(),
                &orig,
                user_id,
                user_args,
                args.order,
                tz,
                legacy_scores,
            )
            .await
            .wrap_err("failed to create top graph")?
        }
    };

    let Some((user, graph)) = tuple_option else {
        return Ok(());
    };

    let embed = EmbedBuilder::new()
        .author(user.author_builder())
        .image(attachment("graph.png"));

    let builder = MessageBuilder::new()
        .embed(embed)
        .attachment("graph.png", graph);

    orig.create_message(&ctx, builder).await?;

    Ok(())
}

const W: u32 = 1350;
const H: u32 = 711;

async fn top_graph(
    ctx: Arc<Context>,
    orig: &CommandOrigin<'_>,
    user_id: UserId,
    user_args: UserArgs,
    order: GraphTopOrder,
    tz: Option<UtcOffset>,
    legacy_scores: bool,
) -> Result<Option<(RedisData<User>, Vec<u8>)>> {
    let scores_fut = ctx
        .osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec_with_user(user_args);

    let (user, mut scores) = match scores_fut.await {
        Ok(tuple) => tuple,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;
            orig.error(&ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    if scores.is_empty() {
        let content = "User's top scores are empty";
        orig.error(&ctx, content).await?;

        return Ok(None);
    }

    let (username, country_code, mode) = match &user {
        RedisData::Original(user) => {
            let username = user.username.as_str();
            let country_code = user.country_code.as_str();
            let mode = user.mode;

            (username, country_code, mode)
        }
        RedisData::Archive(user) => {
            let username = user.username.as_str();
            let country_code = user.country_code.as_str();
            let mode = user.mode;

            (username, country_code, mode)
        }
    };

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
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;
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

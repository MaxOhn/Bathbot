use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::Report;
use rosu_v2::prelude::OsuError;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, ProfileEmbed},
    pagination::ProfilePagination,
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder, constants::OSU_API_ISSUE, matcher, ApplicationCommandExt,
        ChannelExt, CowUtils,
    },
    BotResult, Context,
};

pub use self::{
    data::{ProfileData, ProfileResult},
    graph::graphs,
    graph::{ProfileGraphFlags, ProfileGraphParams},
    size::ProfileEmbedMap,
};

use super::{get_user_and_scores, ScoreArgs, UserArgs};

mod data;
mod graph;
mod size;

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(name = "profile")]
/// Display statistics of a user
pub struct Profile<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(help = "Specify the initial size of the embed.\n\
        If none is specified, it will pick the size as configured with the `/config` command.\n\
        If none is configured, it defaults to `compact`.")]
    /// Choose an embed size
    size: Option<ProfileSize>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
pub enum ProfileSize {
    #[option(name = "Compact", value = "compact")]
    Compact,
    #[option(name = "Medium", value = "medium")]
    Medium,
    #[option(name = "Full", value = "full")]
    Full,
}

impl<'m> Profile<'m> {
    fn args(mode: GameModeOption, args: Args<'m>) -> Result<Self, String> {
        let mut name = None;
        let mut discord = None;
        let mut size = None;

        for arg in args.map(|arg| arg.cow_to_ascii_lowercase()) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = &arg[idx + 1..];

                match key {
                    "size" => {
                        size = match value {
                            "compact" | "small" => Some(ProfileSize::Compact),
                            "medium" => Some(ProfileSize::Medium),
                            "full" | "big" => Some(ProfileSize::Full),
                            _ => {
                                let content = "Failed to parse `size`. Must be either `compact`, `medium`, or `full`.";

                                return Err(content.to_owned());
                            }
                        };
                    }
                    _ => {
                        let content =
                            format!("Unrecognized option `{key}`.\nAvailable options are: `size`.");

                        return Err(content);
                    }
                }
            } else if let Some(id) = matcher::get_mention_user(&arg) {
                discord = Some(id);
            } else {
                name = Some(arg);
            }
        }

        Ok(Self {
            mode: Some(mode),
            name,
            size,
            discord,
        })
    }
}

#[command]
#[desc("Display statistics of a user")]
#[help(
    "Display statistics of a user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise with the `config` command."
)]
#[usage("[username] [size=compact/medium/full]")]
#[examples("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[alias("profile")]
#[group(Osu)]
async fn prefix_osu(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Profile::args(GameModeOption::Osu, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display statistics of a mania user")]
#[help(
    "Display statistics of a mania user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise with the `config` command."
)]
#[usage("[username] [size=compact/medium/full]")]
#[examples("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profilemania", "maniaprofile", "profilem")]
#[group(Mania)]
async fn prefix_mania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Profile::args(GameModeOption::Mania, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display statistics of a taiko user")]
#[help(
    "Display statistics of a taiko user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise with the `config` command."
)]
#[usage("[username] [size=compact/medium/full]")]
#[examples("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profiletaiko", "taikoprofile", "profilet")]
#[group(Taiko)]
async fn prefix_taiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Profile::args(GameModeOption::Taiko, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display statistics of a ctb user")]
#[help(
    "Display statistics of a ctb user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise with the `config` command."
)]
#[usage("[username] [size=compact/medium/full]")]
#[examples("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profilectb", "ctbprofile", "profilec")]
#[group(Catch)]
async fn prefix_ctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Profile::args(GameModeOption::Catch, args) {
        Ok(args) => profile(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_profile(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Profile::from_interaction(command.input_data())?;

    profile(ctx, command.into(), args).await
}

async fn profile(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Profile<'_>) -> BotResult<()> {
    let (name, mode) = name_mode!(ctx, orig, args);
    let size = args.size;
    let guild = orig.guild_id();

    let kind = match (size, guild) {
        (Some(kind), _) => kind,
        (None, Some(guild)) => ctx.guild_profile_size(guild).await,
        (None, None) => ProfileSize::default(),
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100);

    let (user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((mut user, scores)) => {
            user.mode = mode;

            (user, scores)
        }
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    let tracking_fut = process_osu_tracking(&ctx, &mut scores, Some(&user));

    // Try to get the user discord id that is linked to the osu!user
    let discord_id_fut = ctx.psql().get_discord_from_osu_id(user.user_id);

    let discord_id = match tokio::join!(discord_id_fut, tracking_fut) {
        (Ok(user), _) => guild
            .zip(user)
            .filter(|&(guild, user)| ctx.cache.member(guild, user, |_| ()).is_ok())
            .map(|(_, user)| user),
        (Err(err), _) => {
            let report = Report::new(err).wrap_err("failed to get discord id from osu! user id");
            warn!("{report:?}");

            None
        }
    };

    let mut profile_data = ProfileData::new(user, scores, discord_id);

    // Draw the graph
    let params = ProfileGraphParams::new(&ctx, &mut profile_data.user);

    let graph = match graphs(params).await {
        Ok(graph_option) => graph_option,
        Err(err) => {
            warn!("{:?}", Report::new(err));

            None
        }
    };

    // Create the embed
    let embed_data = ProfileEmbed::get_or_create(&ctx, kind, &mut profile_data).await;

    // Send the embed
    let embed = embed_data.as_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph {
        builder = builder.attachment("profile_graph.png", bytes);
    }

    let response = orig.create_message(&ctx, &builder).await?.model().await?;

    // Pagination
    let pagination = ProfilePagination::new(response, profile_data, kind);
    let owner = orig.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

impl ProfileEmbed {
    #[allow(clippy::needless_lifetimes)]
    pub async fn get_or_create<'d>(
        ctx: &Context,
        kind: ProfileSize,
        profile_data: &'d mut ProfileData,
    ) -> &'d Self {
        if profile_data.embeds.get(kind).is_none() {
            let user = &profile_data.user;

            let data = match kind {
                ProfileSize::Compact => {
                    let max_pp = profile_data
                        .scores
                        .first()
                        .and_then(|score| score.pp)
                        .unwrap_or(0.0);

                    ProfileEmbed::compact(user, max_pp, profile_data.discord_id)
                }
                ProfileSize::Medium => {
                    let scores = &profile_data.scores;

                    if profile_data.profile_result.is_none() && !scores.is_empty() {
                        let stats = user.statistics.as_ref().unwrap();

                        profile_data.profile_result =
                            Some(ProfileResult::calc(user.mode, scores, stats));
                    }

                    let bonus_pp = profile_data
                        .profile_result
                        .as_ref()
                        .map_or(0.0, |result| result.bonus_pp);

                    ProfileEmbed::medium(user, bonus_pp, profile_data.discord_id)
                }
                ProfileSize::Full => {
                    let scores = &profile_data.scores;
                    let mode = user.mode;
                    let own_top_scores = profile_data.own_top_scores();

                    let globals_count = match profile_data.globals_count.as_ref() {
                        Some(counts) => counts,
                        None => match super::get_globals_count(ctx, user, mode).await {
                            Ok(globals_count) => profile_data.globals_count.insert(globals_count),
                            Err(why) => {
                                let report =
                                    Report::new(why).wrap_err("failed to request globals count");
                                warn!("{report:?}");

                                profile_data.globals_count.insert(BTreeMap::new())
                            }
                        },
                    };

                    if profile_data.profile_result.is_none() && !scores.is_empty() {
                        let stats = user.statistics.as_ref().unwrap();

                        profile_data.profile_result =
                            Some(ProfileResult::calc(mode, scores, stats));
                    }

                    let profile_result = profile_data.profile_result.as_ref();

                    ProfileEmbed::full(
                        user,
                        profile_result,
                        globals_count,
                        own_top_scores,
                        profile_data.discord_id,
                    )
                }
            };

            profile_data.embeds.insert(kind, data);
        }

        // Annoying NLL workaround; TODO: Fix when possible
        //   - https://github.com/rust-lang/rust/issues/43234
        //   - https://github.com/rust-lang/rust/issues/51826
        profile_data.embeds.get(kind).unwrap()
    }
}

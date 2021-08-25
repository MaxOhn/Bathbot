mod args;
mod data;
mod graph;
mod size;

use args::ProfileArgs;
pub use data::{ProfileData, ProfileResult};
use graph::graphs;
pub use size::{ProfileEmbedMap, ProfileSize};

use crate::{
    embeds::{EmbedData, ProfileEmbed},
    pagination::ProfilePagination,
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{collections::BTreeMap, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice,
    },
    interaction::ApplicationCommand,
};

async fn _profile(ctx: Arc<Context>, data: CommandData<'_>, args: ProfileArgs) -> BotResult<()> {
    let ProfileArgs { name, mode, kind } = args;

    let author_id = data.author()?.id;

    let config = match ctx.user_config(author_id).await {
        Ok(config) => config,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let kind = kind.unwrap_or(config.profile_embed_size);

    let mode = match (mode, config.mode) {
        (GameMode::MNA | GameMode::TKO | GameMode::CTB, _) => mode,
        (_, GameMode::MNA | GameMode::TKO | GameMode::CTB) => config.mode,
        _ => GameMode::STD,
    };

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(author_id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    // Retrieve the user and their top scores
    let user_fut = super::request_user(&ctx, &name, Some(mode));
    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((mut user, scores)) => {
            user.mode = mode;

            (user, scores)
        }
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    // Store maps in DB
    if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
        unwind_error!(warn, why, "Error while storing profile maps in DB: {}");
    }

    let mut profile_data = ProfileData::new(user, scores);

    // Draw the graph
    let graph = match graphs(&mut profile_data.user).await {
        Ok(graph_option) => graph_option,
        Err(why) => {
            unwind_error!(warn, why, "Error while creating profile graph: {}");

            None
        }
    };

    // Create the embed
    let embed_data = ProfileEmbed::get_or_create(&ctx, kind, &mut profile_data).await;

    // Send the embed
    let embed = embed_data.as_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph.as_deref() {
        builder = builder.file("profile_graph.png", bytes);
    }

    let response = data.create_message(&ctx, builder).await?.model().await?;

    // Pagination
    let pagination = ProfilePagination::new(response, profile_data, kind);
    let owner = author_id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (profile): {}")
        }
    });

    Ok(())
}

impl ProfileEmbed {
    pub async fn get_or_create<'map>(
        ctx: &Context,
        kind: ProfileSize,
        profile_data: &'map mut ProfileData,
    ) -> &'map Self {
        if profile_data.embeds.get(kind).is_none() {
            let user = &profile_data.user;

            let data = match kind {
                ProfileSize::Compact => {
                    let max_pp = profile_data
                        .scores
                        .first()
                        .and_then(|score| score.pp)
                        .unwrap_or(0.0);

                    ProfileEmbed::compact(user, max_pp)
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

                    ProfileEmbed::medium(user, bonus_pp)
                }
                ProfileSize::Full => {
                    let scores = &profile_data.scores;
                    let mode = user.mode;
                    let own_top_scores = profile_data.own_top_scores();

                    let globals_count = match profile_data.globals_count.as_ref() {
                        Some(counts) => counts,
                        None => match super::get_globals_count(ctx, &user.username, mode).await {
                            Ok(globals_count) => profile_data.globals_count.insert(globals_count),
                            Err(why) => {
                                unwind_error!(
                                    error,
                                    why,
                                    "Error while requesting globals count: {}"
                                );

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

                    ProfileEmbed::full(&user, profile_result, globals_count, own_top_scores)
                }
            };

            profile_data.embeds.insert(kind, data);
        }

        // Annoying NLL workaround
        //   - https://github.com/rust-lang/rust/issues/43234
        //   - https://github.com/rust-lang/rust/issues/51826
        profile_data.embeds.get(kind).unwrap()
    }
}

#[command]
#[short_desc("Display statistics of a user")]
#[long_desc(
    "Display statistics of a user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise in the user config."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profile")]
async fn osu(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

#[command]
#[short_desc("Display statistics of a mania user")]
#[long_desc(
    "Display statistics of a mania user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise in the user config."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profilemania", "maniaprofile", "profilem")]
async fn mania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

#[command]
#[short_desc("Display statistics of a taiko user")]
#[long_desc(
    "Display statistics of a taiko user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise in the user config."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profiletaiko", "taikoprofile", "profilet")]
async fn taiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

#[command]
#[short_desc("Display statistics of a ctb user")]
#[long_desc(
    "Display statistics of a ctb user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`.\n\
    Defaults to `compact` if not specified otherwise in the user config."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profilectb", "ctbprofile", "profilec")]
async fn ctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

pub async fn slash_profile(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match ProfileArgs::slash(&ctx, &mut command)? {
        Ok(args) => _profile(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_profile_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "profile".to_owned(),
        default_permission: None,
        description: "Display statistics of a user".to_owned(),
        id: None,
        options: vec![
            CommandOption::String(ChoiceCommandOptionData {
                choices: super::mode_choices(),
                description: "Specify a gamemode".to_owned(),
                name: "mode".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![
                    CommandOptionChoice::String {
                        name: "compact".to_owned(),
                        value: "compact".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "medium".to_owned(),
                        value: "medium".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "full".to_owned(),
                        value: "full".to_owned(),
                    },
                ],
                description: "Choose an embed size".to_owned(),
                name: "size".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify a username".to_owned(),
                name: "name".to_owned(),
                required: false,
            }),
            CommandOption::User(BaseCommandOptionData {
                description: "Specify a linked discord user".to_owned(),
                name: "discord".to_owned(),
                required: false,
            }),
        ],
    }
}

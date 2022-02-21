use std::sync::Arc;

use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError, Username};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        parse_discord, parse_mode_option, DoubleResultCow, MyCommand, MyCommandOption,
    },
    database::UserConfig,
    embeds::{EmbedData, TopEmbed},
    pagination::{Pagination, TopPagination},
    tracking::process_osu_tracking,
    util::{
        constants::{
            common_literals::{DISCORD, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher, numbers, ApplicationCommandExt, CowUtils, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

use super::{option_discord, option_mode, option_name};

async fn _mapper(ctx: Arc<Context>, data: CommandData<'_>, args: MapperArgs) -> BotResult<()> {
    let MapperArgs { config, mapper } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let user = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let mapper = mapper.cow_to_ascii_lowercase();

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(user.as_str(), mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{user}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let scores: Vec<_> = scores
        .into_iter()
        .enumerate()
        .filter(|(_, score)| {
            let map = &score.map.as_ref().unwrap();
            let mapset = &score.mapset.as_ref().unwrap();

            //  Filter converts
            if map.mode != mode {
                return false;
            }

            // Either the version contains the mapper name (guest diff'd by mapper)
            // or the map is created by mapper name and not guest diff'd by someone else
            let version = map.version.to_lowercase();

            version.contains(mapper.as_ref())
                || (mapset.creator_name.to_lowercase().as_str() == mapper.as_ref()
                    && !matcher::is_guest_diff(&version))
        })
        .collect();

    // Accumulate all necessary data
    let content = match mapper.as_ref() {
        "sotarks" => {
            let amount = scores.len();

            let mut content = format!(
                "I found {amount} Sotarks map{plural} in `{name}`'s top100, ",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
                name = user.username,
            );

            let to_push = match amount {
                0 => "I'm proud \\:)",
                1..=4 => "that's already too many...",
                5..=8 => "kinda sad \\:/",
                9..=15 => "pretty sad \\:(",
                16..=25 => "this is so sad \\:((",
                26..=35 => "this needs to stop this",
                36..=49 => "that's a serious problem...",
                50 => "that's half. HALF.",
                51..=79 => "how do you sleep at night...",
                80..=89 => "so close to ultimate disaster...",
                90..=99 => "i'm not even mad, that's just impressive",
                100 => "you did it. \"Congrats\".",
                _ => "wait how did you do that",
            };

            content.push_str(to_push);

            content
        }
        _ => format!(
            "{} of `{}`'{} top score maps were mapped by `{mapper}`",
            scores.len(),
            user.username,
            if user.username.ends_with('s') {
                ""
            } else {
                "s"
            },
        ),
    };

    let builder = if scores.is_empty() {
        MessageBuilder::new().embed(content)
    } else {
        let pages = numbers::div_euclid(5, scores.len());
        let data = TopEmbed::new(&user, scores.iter().take(5), (1, pages)).await;
        let embed = data.into_builder().build();

        MessageBuilder::new().content(content).embed(embed)
    };

    let response_raw = data.create_message(&ctx, builder).await?;

    let scores_iter = scores.iter().map(|(_, score)| score);

    // Store maps of scores in DB; combo was inserted earlier
    if let Err(err) = ctx.psql().store_scores_maps(scores_iter).await {
        warn!("{:?}", Report::new(err));
    }

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = TopPagination::new(response, user, scores);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[command]
#[short_desc("How many maps of a user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username] [mapper]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
pub async fn mapper(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match MapperArgs::args(&ctx, &mut args, msg.author.id, None).await {
                Ok(Ok(mut mapper_args)) => {
                    mapper_args.config.mode.get_or_insert(GameMode::STD);

                    _mapper(ctx, CommandData::Message { msg, args, num }, mapper_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("How many maps of a mania user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a mania user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native mania maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[aliases("mapperm")]
pub async fn mappermania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match MapperArgs::args(&ctx, &mut args, msg.author.id, None).await {
                Ok(Ok(mut mapper_args)) => {
                    mapper_args.config.mode = Some(GameMode::MNA);

                    _mapper(ctx, CommandData::Message { msg, args, num }, mapper_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("How many maps of a taiko user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a taiko user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native taiko maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[aliases("mappert")]
pub async fn mappertaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match MapperArgs::args(&ctx, &mut args, msg.author.id, None).await {
                Ok(Ok(mut mapper_args)) => {
                    mapper_args.config.mode = Some(GameMode::TKO);

                    _mapper(ctx, CommandData::Message { msg, args, num }, mapper_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("How many maps of a ctb user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a ctb user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native ctb maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[aliases("mapperc")]
async fn mapperctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match MapperArgs::args(&ctx, &mut args, msg.author.id, None).await {
                Ok(Ok(mut mapper_args)) => {
                    mapper_args.config.mode = Some(GameMode::CTB);

                    _mapper(ctx, CommandData::Message { msg, args, num }, mapper_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("How many maps of a user's top100 are made by Sotarks?")]
#[long_desc(
    "How many maps of a user's top100 are made by Sotarks?\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username]")]
#[example("badewanne3")]
pub async fn sotarks(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match MapperArgs::args(&ctx, &mut args, msg.author.id, Some("sotarks")).await {
                Ok(Ok(mut mapper_args)) => {
                    mapper_args.config.mode.get_or_insert(GameMode::STD);

                    _mapper(ctx, CommandData::Message { msg, args, num }, mapper_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

pub async fn slash_mapper(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let options = command.yoink_options();

    match MapperArgs::slash(&ctx, &command, options).await? {
        Ok(args) => _mapper(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

struct MapperArgs {
    config: UserConfig,
    mapper: Username,
}

impl MapperArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
        mapper: Option<&str>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;

        let (name, mapper) = match args.next() {
            Some(first) => match mapper {
                Some(mapper) => (Some(first), mapper),
                None => match args.next() {
                    Some(second) => (Some(first), second),
                    None => (None, first),
                },
            },
            None => match mapper {
                Some(mapper) => (None, mapper),
                None => {
                    let content = "You need to specify at least one osu username for the mapper. \
                        If you're not linked, you must specify at least two names.";

                    return Ok(Err(content.into()));
                }
            },
        };

        if let Some(name) = name {
            match check_user_mention(ctx, name).await? {
                Ok(osu) => config.osu = Some(osu),
                Err(content) => return Ok(Err(content)),
            }
        }

        let mapper = match check_user_mention(ctx, mapper).await? {
            Ok(osu) => osu.into_username(),
            Err(content) => return Ok(Err(content)),
        };

        Ok(Ok(Self { config, mapper }))
    }

    async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut mapper = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    "mapper" => mapper = Some(value.into()),
                    MODE => config.mode = parse_mode_option(&value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => config.osu = Some(osu),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let args = Self {
            mapper: mapper.ok_or(Error::InvalidCommandOptions)?,
            config,
        };

        Ok(Ok(args))
    }
}

pub fn define_mapper() -> MyCommand {
    let mapper =
        MyCommandOption::builder("mapper", "Specify a mapper username").string(Vec::new(), false);

    let mode = option_mode();
    let name = option_name();
    let discord = option_discord();

    let mapper_help = "Count the top plays on maps of the given mapper.\n\
        It will try to consider guest difficulties so that if a map was created by someone else \
        but the given mapper made the guest diff, it will count.\n\
        Similarly, if the given mapper created the mapset but someone else guest diff'd, \
        it will not count.\n\
        This does not always work perfectly, like when mappers renamed or when guest difficulties don't have \
        common difficulty labels like `X's Insane`";

    MyCommand::new("mapper", "Count the top plays on maps of the given mapper")
        .help(mapper_help)
        .options(vec![mapper, mode, name, discord])
}

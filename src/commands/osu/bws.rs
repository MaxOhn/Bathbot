use crate::{
    commands::SlashCommandBuilder,
    database::UserConfig,
    embeds::{BWSEmbed, EmbedData},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{borrow::Cow, cmp::Ordering, sync::Arc};
use twilight_model::{
    application::{
        command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    id::UserId,
};

const MIN_BADGES_OFFSET: usize = 2;

#[command]
#[short_desc("Show the badge weighted seeding for a player")]
#[long_desc(
    "Show the badge weighted seeding for a player. \n\
    The current formula is `rank^(0.9937^(badges^2))`.\n\
    Next to the player's username, you can specify `rank=integer` \
    to show how the bws value progresses towards that rank.\n\
    Similarly, you can specify `badges=integer` to show how the value \
    progresses towards that badge amount."
)]
#[usage("[username] [rank=integer] [badges=integer]")]
#[example("badewanne3", "badewanne3 rank=1234 badges=10", "badewanne3 badges=3")]
async fn bws(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match BwsArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(bws_args)) => {
                    _bws(ctx, CommandData::Message { msg, args, num }, bws_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => slash_bws(ctx, *command).await,
    }
}

async fn _bws(ctx: Arc<Context>, data: CommandData<'_>, args: BwsArgs) -> BotResult<()> {
    let BwsArgs {
        config,
        rank,
        badges,
    } = args;

    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.osu_username {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user = match super::request_user(&ctx, &name, Some(mode)).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let badges_curr = user
        .badges
        .as_ref()
        .unwrap()
        .iter()
        .filter(|badge| matcher::tourney_badge(badge.description.as_str()))
        .count();

    let (badges_min, badges_max) = match badges {
        Some(num) => match num.cmp(&badges_curr) {
            Ordering::Less => {
                if badges_curr >= MIN_BADGES_OFFSET {
                    (num, badges_curr)
                } else {
                    (0, MIN_BADGES_OFFSET)
                }
            }
            Ordering::Equal => (badges_curr, badges_curr + MIN_BADGES_OFFSET),
            Ordering::Greater => (badges_curr, num.max(badges_curr + MIN_BADGES_OFFSET)),
        },
        None => (badges_curr, badges_curr + MIN_BADGES_OFFSET),
    };

    let embed_data = BWSEmbed::new(user, badges_curr, badges_min, badges_max, rank);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

struct BwsArgs {
    config: UserConfig,
    rank: Option<u32>,
    badges: Option<usize>,
}

impl BwsArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut rank = None;
        let mut badges = None;

        for arg in args.take(3) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "rank" | "r" => match value.parse::<u32>() {
                        Ok(num) => rank = Some(num.max(1)),
                        Err(_) => {
                            let content = "Failed to parse `rank`. Must be a positive integer.";

                            return Ok(Err(content.into()));
                        }
                    },
                    "badges" | "badge" | "b" => match value.parse() {
                        Ok(num) => badges = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `badges`. Must be a positive integer.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `rank` or `badges`.",
                            key
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else {
                match Args::check_user_mention(ctx, arg).await? {
                    Ok(name) => config.osu_username = Some(name),
                    Err(content) => return Ok(Err(content.into())),
                }
            }
        }

        let args = Self {
            config,
            rank,
            badges,
        };

        Ok(Ok(args))
    }

    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, String>> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut rank = None;
        let mut badges = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "name" => config.osu_username = Some(value.into()),
                    "discord" => config.osu_username = parse_discord_option!(ctx, value, "bws"),
                    _ => bail_cmd_option!("bws", string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    "rank" => rank = Some(value.max(1) as u32),
                    "badges" => badges = Some(value.max(0) as usize),
                    _ => bail_cmd_option!("bws", integer, name),
                },
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("bws", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("bws", subcommand, name)
                }
            }
        }

        let args = Self {
            config,
            rank,
            badges,
        };

        Ok(Ok(args))
    }
}

pub async fn slash_bws(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match BwsArgs::slash(&ctx, &mut command).await? {
        Ok(args) => _bws(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_bws_command() -> Command {
    let description = "Show the badge weighted seeding for an osu!standard player";

    let options = vec![
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![],
            description: "Specify a username".to_owned(),
            name: "name".to_owned(),
            required: false,
        }),
        CommandOption::Integer(ChoiceCommandOptionData {
            choices: vec![],
            description: "Specify a target rank to reach".to_owned(),
            name: "rank".to_owned(),
            required: false,
        }),
        CommandOption::Integer(ChoiceCommandOptionData {
            choices: vec![],
            description: "Specify an amount of badges to reach".to_owned(),
            name: "badges".to_owned(),
            required: false,
        }),
        CommandOption::User(BaseCommandOptionData {
            description: "Specify a linked discord user".to_owned(),
            name: "discord".to_owned(),
            required: false,
        }),
    ];

    SlashCommandBuilder::new("bws", description)
        .options(options)
        .build()
}

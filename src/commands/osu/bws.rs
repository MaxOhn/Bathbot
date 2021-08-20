use crate::{
    embeds::{BWSEmbed, EmbedData},
    util::{constants::OSU_API_ISSUE, matcher, ApplicationCommandExt, MessageExt},
    Args, BotResult, CommandData, Context, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{borrow::Cow, cmp::Ordering, sync::Arc};
use twilight_model::application::{
    command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
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
        CommandData::Message { msg, mut args, num } => match BwsArgs::args(&ctx, &mut args) {
            Ok(bws_args) => _bws(ctx, CommandData::Message { msg, args, num }, bws_args).await,
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_bws(ctx, command).await,
    }
}

async fn _bws(ctx: Arc<Context>, data: CommandData<'_>, args: BwsArgs) -> BotResult<()> {
    let BwsArgs { name, rank, badges } = args;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    let mode = GameMode::STD;

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
    name: Option<Name>,
    rank: Option<u32>,
    badges: Option<usize>,
}

impl BwsArgs {
    fn args(ctx: &Context, args: &mut Args) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
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

                            return Err(content.into());
                        }
                    },
                    "badges" | "badge" | "b" => match value.parse() {
                        Ok(num) => badges = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `badges`. Must be a positive integer.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `rank` or `badges`.",
                            key
                        );

                        return Err(content.into());
                    }
                }
            } else {
                name = Some(Args::try_link_name(ctx, arg)?);
            }
        }

        Ok(Self { name, rank, badges })
    }

    fn slash(ctx: &Context, command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut rank = None;
        let mut badges = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!(ctx, value, "bws"),
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
            name: username,
            rank,
            badges,
        };

        Ok(Ok(args))
    }
}

pub async fn slash_bws(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match BwsArgs::slash(&ctx, &mut command)? {
        Ok(args) => _bws(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_bws_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "bws".to_owned(),
        default_permission: None,
        description: "Show the badge weighted seeding for an osu!standard player".to_owned(),
        id: None,
        options: vec![
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
        ],
    }
}

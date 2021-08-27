use crate::{
    database::UserConfig,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, ApplicationCommandExt, MessageBuilder, MessageExt,
    },
    Args, BotResult, CommandData, Context, Name,
};

use rosu_v2::error::OsuError;
use std::{fmt::Write, sync::Arc};
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption, OptionsCommandOptionData},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

struct LinkArgs {
    arg: Option<Name>,
}

impl LinkArgs {
    fn args(args: &mut Args) -> Result<Self, String> {
        let content = args.rest();

        if !(content.starts_with('"') && content.ends_with('"')) && content.contains(' ') {
            let suggestion = format!(
                "Usernames containing whitespace must be encapsulated with quotation marks.\n\
                Did you mean `\"{}\"`?",
                content
            );

            Err(suggestion)
        } else {
            let arg = args.next().map(Name::from);

            Ok(Self { arg })
        }
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
        let mut username = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("link", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("link", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("link", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "user" => {
                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => username = Some(value.into()),
                                    _ => bail_cmd_option!("link", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("link", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("link", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("link", subcommand, name)
                                }
                            }
                        }
                    }
                    "remove" => username = None,
                    _ => bail_cmd_option!("link", subcommand, name),
                },
            }
        }

        Ok(Ok(Self { arg: username }))
    }
}

#[command]
#[short_desc("Link your discord to an osu profile")]
#[long_desc(
    "Link your discord account to an osu name. \n\
     Don't forget the `\"` if the name contains whitespace.\n\
     If no arguments are provided, I will unlink \
     your discord account from any osu name."
)]
#[usage("[username / url to user profile]")]
#[example("badewanne3", "\"nathan on osu\"", "https://osu.ppy.sh/users/2211396")]
async fn link(ctx: Arc<Context>, mut data: CommandData) -> BotResult<()> {
    let args = match &mut data {
        CommandData::Message { args, .. } => LinkArgs::args(args),
        CommandData::Interaction { command } => LinkArgs::slash(command)?,
    };

    let author = data.author()?;

    let name = match args {
        Ok(LinkArgs { arg: Some(arg) }) => match matcher::get_osu_user_id(arg.as_str()) {
            Some(id) => match ctx.osu().user(id).await {
                Ok(user) => user.username.into(),
                Err(OsuError::NotFound) => {
                    let content = format!("No user with the id `{}` was found.", id);

                    return data.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            },
            None => arg,
        },
        Ok(LinkArgs { arg: None }) => {
            let mut config = match ctx.psql().get_user_config(author.id).await {
                Ok(Some(config)) => config,
                Ok(None) => UserConfig::default(),
                Err(why) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(why);
                }
            };

            config.name.take();

            if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            let builder = MessageBuilder::new().content("You are no longer linked");
            data.create_message(&ctx, builder).await?;

            return Ok(());
        }
        Err(suggestion) => return data.error(&ctx, suggestion).await,
    };

    if name.chars().count() > 15 {
        let content = "That name is too long, must be at most 15 characters";

        return data.error(&ctx, content).await;
    }

    let user = match super::request_user(&ctx, name.as_str(), None).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let mut content = format!("No user with the name `{}` was found.", &name);

            if name.contains('_') {
                let _ = write!(
                    content,
                    "\nIf the name contains whitespace, be sure to encapsulate \
                    it inbetween quotation marks, e.g `\"{}\"`.",
                    name.replace('_', " "),
                );
            }

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let mut config = match ctx.psql().get_user_config(author.id).await {
        Ok(Some(config)) => config,
        Ok(None) => UserConfig::default(),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    config.name = Some(user.username.as_str().into());

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = data.error(&ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let content = format!(
        "I linked discord's `{}` with osu's `{}`",
        author.name, user.username
    );

    let builder = MessageBuilder::new().content(content);
    let _ = data.create_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_link(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    link(ctx, command.into()).await
}

pub fn slash_link_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "link".to_owned(),
        default_permission: None,
        description: "(Un)link your discord to an osu profile".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Link your discord to an osu profile".to_owned(),
                name: "user".to_owned(),
                options: vec![CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "The osu! username to link to".to_owned(),
                    name: "name".to_owned(),
                    required: true,
                })],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Unlink your discord from osu".to_owned(),
                name: "remove".to_owned(),
                options: vec![],
                required: false,
            }),
        ],
    }
}

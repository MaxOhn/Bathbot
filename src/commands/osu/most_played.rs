use crate::{
    embeds::{EmbedData, MostPlayedEmbed},
    pagination::{MostPlayedPagination, Pagination},
    util::{constants::OSU_API_ISSUE, numbers, ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context, Name,
};

use rosu_v2::prelude::OsuError;
use std::sync::Arc;
use twilight_model::application::{
    command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Display the most played maps of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mp")]
async fn mostplayed(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = args.next().map(Name::from);
            let data = CommandData::Message { msg, args, num };

            _mostplayed(ctx, data, name).await
        }
        CommandData::Interaction { command } => slash_mostplayed(ctx, command).await,
    }
}

async fn _mostplayed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Name>,
) -> BotResult<()> {
    let author_id = data.author()?.id;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(author_id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    // Retrieve the user and their most played maps
    let user_fut = super::request_user(&ctx, &name, None);
    let maps_fut_1 = ctx.osu().user_most_played(name.as_str()).limit(50);
    let maps_fut_2 = ctx
        .osu()
        .user_most_played(name.as_str())
        .limit(50)
        .offset(50);

    let (user, maps) = match tokio::try_join!(user_fut, maps_fut_1, maps_fut_2) {
        Ok((user, mut maps, mut maps_2)) => {
            maps.append(&mut maps_2);

            (user, maps)
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

    // Accumulate all necessary data
    let pages = numbers::div_euclid(10, maps.len());
    let embed_data = MostPlayedEmbed::new(&user, maps.iter().take(10), (1, pages));

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MostPlayedPagination::new(response, user, maps);
    let owner = author_id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (mostplayed): {}")
        }
    });

    Ok(())
}

pub async fn slash_mostplayed(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut username = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "name" => username = Some(value.into()),
                "discord" => match value.parse() {
                    Ok(id) => match ctx.get_link(id) {
                        Some(name) => username = Some(name),
                        None => {
                            let content = format!("<@{}> is not linked to an osu profile", id);

                            return command.error(&ctx, content).await;
                        }
                    },
                    Err(_) => {
                        bail_cmd_option!("mostplayed discord", string, value)
                    }
                },
                _ => bail_cmd_option!("mostplayed", string, name),
            },
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!("mostplayed", integer, name)
            }
            CommandDataOption::Boolean { name, .. } => {
                bail_cmd_option!("mostplayed", boolean, name)
            }
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("mostplayed", subcommand, name)
            }
        }
    }

    _mostplayed(ctx, command.into(), username).await
}

pub fn slash_mostplayed_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "mostplayed".to_owned(),
        default_permission: None,
        description: "Display the most played maps of a user".to_owned(),
        id: None,
        options: vec![
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

use crate::{
    embeds::{EmbedData, RatioEmbed},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, ApplicationCommandExt, MessageExt},
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::application::{
    command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Ratio related stats about a user's top100")]
#[long_desc(
    "Calculate the average ratios of a user's top100.\n\
    If the command was used before on the given osu name, \
    I will also compare the current results with the ones from last time \
    if they've changed since."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("ratio")]
async fn ratios(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match args
                .next()
                .map(|arg| Args::try_link_name(&ctx, arg))
                .transpose()
            {
                Ok(name) => _ratios(ctx, CommandData::Message { msg, args, num }, name).await,
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_ratio(ctx, command).await,
    }
}

async fn _ratios(ctx: Arc<Context>, data: CommandData<'_>, name: Option<Name>) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    // Retrieve the user and their top scores
    let user_fut = super::request_user(&ctx, &name, Some(GameMode::MNA));

    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(GameMode::MNA)
        .limit(100);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => (user, scores),
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
    process_tracking(&ctx, GameMode::MNA, &mut scores, Some(&user)).await;

    // Accumulate all necessary data
    let embed_data = RatioEmbed::new(user, scores);
    let content = format!("Average ratios of `{}`'s top 100 in mania:", name);

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

fn parse_username(
    ctx: &Context,
    command: &mut ApplicationCommand,
) -> BotResult<Result<Option<Name>, String>> {
    let mut username = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "name" => username = Some(value.into()),
                "discord" => username = parse_discord_option!(ctx, value, "ratios"),
                _ => bail_cmd_option!("ratios", string, name),
            },
            CommandDataOption::Integer { name, .. } => bail_cmd_option!("ratios", integer, name),
            CommandDataOption::Boolean { name, .. } => bail_cmd_option!("ratios", boolean, name),
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("ratios", subcommand, name)
            }
        }
    }

    Ok(Ok(username))
}

pub async fn slash_ratio(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match parse_username(&ctx, &mut command)? {
        Ok(name) => _ratios(ctx, command.into(), name).await,
        Err(content) => return command.error(&ctx, content).await,
    }
}

pub fn slash_ratio_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "ratios".to_owned(),
        default_permission: None,
        description: "Ratio related stats about a user's mania top100".to_owned(),
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

use crate::{
    embeds::{EmbedData, RatioEmbed},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, ApplicationCommandExt, MessageExt},
    Args, BotResult, CommandData, Context, Error, MessageBuilder, Name,
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
        CommandData::Message { msg, mut args, num } => match args.next() {
            Some(arg) => match Args::try_link_name(&ctx, arg) {
                Ok(name) => _ratios(ctx, CommandData::Message { msg, args, num }, name).await,
                Err(content) => msg.error(&ctx, content).await,
            },
            None => match ctx.get_link(msg.author.id.0) {
                Some(name) => _ratios(ctx, CommandData::Message { msg, args, num }, name).await,
                None => super::require_link_msg(&ctx, &msg).await,
            },
        },
        CommandData::Interaction { command } => slash_ratio(ctx, command).await,
    }
}

async fn _ratios(ctx: Arc<Context>, data: CommandData<'_>, name: Name) -> BotResult<()> {
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

pub async fn slash_ratio(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut username = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                "name" => username = Some(value.into()),
                "discord" => {}
                _ => bail_cmd_option!("ratio", string, name),
            },
            CommandDataOption::Integer { name, .. } => bail_cmd_option!("ratio", integer, name),
            CommandDataOption::Boolean { name, .. } => bail_cmd_option!("ratio", boolean, name),
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("ratio", subcommand, name)
            }
        }
    }

    if let Some(resolved) = command.data.resolved.take().filter(|_| username.is_none()) {
        if let Some(user) = resolved.users.first() {
            if let Some(link) = ctx.get_link(user.id.0) {
                username.insert(link);
            } else {
                let content = format!("<@{}> is not linked to an osu profile", user.id);

                return command.error(&ctx, content).await;
            }
        }
    }

    let name = username.ok_or(Error::InvalidCommandOptions)?;

    _ratios(ctx, command.into(), name).await
}

pub fn slash_ratio_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "ratio".to_owned(),
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

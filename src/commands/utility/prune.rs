use crate::{
    commands::SlashCommandBuilder,
    util::{constants::GENERAL_ISSUE, ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context, Error, MessageBuilder,
};

use std::{str::FromStr, sync::Arc};
use tokio::time::{self, Duration};
use twilight_http::{
    api_error::{ApiError, ErrorCode::MessageTooOldToBulkDelete},
    error::ErrorType,
};
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Prune messages in a channel")]
#[long_desc(
    "Optionally provide a number to delete this \
     many of the latest messages of a channel, defaults to 1.\n\
     Amount must be between 1 and 99.\n\
     This command can not delete messages older than 2 weeks."
)]
#[usage("[number]")]
#[example("3")]
#[aliases("purge")]
async fn prune(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let amount = match args.next().map(u64::from_str) {
                Some(Ok(amount)) => {
                    if !(1..100).contains(&amount) {
                        let content = "First argument must be an integer between 1 and 99";

                        return msg.error(&ctx, content).await;
                    }

                    amount + 1
                }
                None | Some(Err(_)) => 2,
            };

            _prune(ctx, CommandData::Message { msg, args, num }, amount).await
        }
        CommandData::Interaction { command } => slash_prune(ctx, *command).await,
    }
}

async fn _prune(ctx: Arc<Context>, data: CommandData<'_>, amount: u64) -> BotResult<()> {
    let channel_id = data.channel_id();

    let msgs_fut = ctx
        .http
        .channel_messages(channel_id)
        .limit(amount)
        .unwrap()
        .exec();

    let mut messages: Vec<_> = match msgs_fut.await {
        Ok(msgs) => msgs
            .models()
            .await?
            .into_iter()
            .take(amount as usize)
            .map(|msg| msg.id)
            .collect(),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why.into());
        }
    };

    if messages.len() < 2 {
        if let Some(msg_id) = messages.pop() {
            ctx.http.delete_message(channel_id, msg_id).exec().await?;
        }

        return Ok(());
    }

    if let Err(why) = ctx.http.delete_messages(channel_id, &messages).exec().await {
        if matches!(why.kind(), ErrorType::Response {
            error: ApiError::General(err),
            ..
        } if err.code == MessageTooOldToBulkDelete)
        {
            let content = "Cannot delete messages that are older than two weeks \\:(";

            return data.error(&ctx, content).await;
        } else {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why.into());
        }
    }

    let content = format!("Deleted the last {} messages", amount);
    let builder = MessageBuilder::new().content(content);
    let response = data.create_message(&ctx, builder).await?.model().await?;
    time::sleep(Duration::from_secs(6)).await;
    response.delete_message(&ctx).await?;

    Ok(())
}

pub async fn slash_prune(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut amount = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, .. } => bail_cmd_option!("prune", string, name),
            CommandDataOption::Integer { name, value } => match name.as_str() {
                "amount" => amount = Some(value.max(1).min(100) as u64),
                _ => bail_cmd_option!("prune", integer, name),
            },
            CommandDataOption::Boolean { name, .. } => bail_cmd_option!("prune", boolean, name),
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("prune", subcommand, name)
            }
        }
    }

    let amount = amount.ok_or(Error::InvalidCommandOptions)?;

    _prune(ctx, command.into(), amount).await
}

pub fn slash_prune_command() -> Command {
    let description = "Delete the last few messages in a channel";

    let options = vec![CommandOption::Integer(ChoiceCommandOptionData {
        choices: vec![],
        description: "Choose the amount of messages to delete".to_owned(),
        name: "amount".to_owned(),
        required: true,
    })];

    SlashCommandBuilder::new("prune", description)
        .options(options)
        .build()
}

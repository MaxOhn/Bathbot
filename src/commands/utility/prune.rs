use crate::{
    commands::{MyCommand, MyCommandOption},
    util::{
        constants::{GENERAL_ISSUE, MESSAGE_TOO_OLD_TO_BULK_DELETE},
        MessageExt,
    },
    BotResult, CommandData, Context, Error, MessageBuilder,
};

use std::{str::FromStr, sync::Arc};
use tokio::time::{self, Duration};
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
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
    let slash = matches!(data, CommandData::Interaction { .. });

    let msgs_fut = ctx
        .http
        .channel_messages(channel_id)
        .limit(amount as u16 + slash as u16)
        .unwrap()
        .exec();

    let mut messages: Vec<_> = match msgs_fut.await {
        Ok(msgs) => msgs
            .models()
            .await?
            .into_iter()
            .skip(slash as usize)
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
        } if err.code == MESSAGE_TOO_OLD_TO_BULK_DELETE)
        {
            let content = "Cannot delete messages that are older than two weeks \\:(";

            return data.error(&ctx, content).await;
        } else {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why.into());
        }
    }

    let content = format!("Deleted the last {} messages", amount - 1 + slash as u64);
    let builder = MessageBuilder::new().content(content);
    let response = data.create_message(&ctx, builder).await?.model().await?;
    time::sleep(Duration::from_secs(6)).await;
    response.delete_message(&ctx).await?;

    Ok(())
}

pub async fn slash_prune(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let option = command.data.options.first().and_then(|option| {
        (option.name == "amount").then(|| match option.value {
            CommandOptionValue::Integer(value) => Some(value),
            _ => None,
        })
    });

    let amount = match option.flatten() {
        Some(value) => value.max(1).min(100) as u64,
        None => return Err(Error::InvalidCommandOptions),
    };

    _prune(ctx, command.into(), amount).await
}

pub fn define_prune() -> MyCommand {
    let amount_help = "Choose the amount of messages to delete. Should be between 1 and 99.";

    let amount = MyCommandOption::builder("amount", "Choose the amount of messages to delete")
        .help(amount_help)
        .min_int(1)
        .integer(Vec::new(), true);

    let help = "Delete the last few messages in a channel.\n\
        Messages older than two weeks __cannot__ be deleted with this command.";

    MyCommand::new("prune", "Delete the last few messages in a channel")
        .help(help)
        .options(vec![amount])
}

use std::{str::FromStr, sync::Arc};

use bathbot_macros::{command, SlashCommand};
use bathbot_util::{
    constants::{GENERAL_ISSUE, MESSAGE_TOO_OLD_TO_BULK_DELETE},
    MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use tokio::time::{self, Duration};
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    core::commands::CommandOrigin,
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt, MessageExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "prune",
    dm_permission = false,
    help = "Delete the last few messages in a channel.\n\
    Messages older than two weeks __cannot__ be deleted with this command."
)]
#[flags(AUTHORITY)]
/// Delete the last few messages in a channel
pub struct Prune {
    #[command(min_value = 1, max_value = 99)]
    /// Choose the amount of messages to delete
    amount: i64,
}
async fn slash_prune(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Prune::from_interaction(command.input_data())?;

    prune(ctx, (&mut command).into(), args.amount as u64).await
}

#[command]
#[desc("Prune messages in a channel")]
#[help(
    "Optionally provide a number to delete this \
     many of the latest messages of a channel, defaults to 1.\n\
     Amount must be between 1 and 99.\n\
     This command can not delete messages older than 2 weeks."
)]
#[usage("[number]")]
#[example("3")]
#[alias("purge")]
#[flags(AUTHORITY, ONLY_GUILDS, SKIP_DEFER)]
#[group(Utility)]
async fn prefix_prune(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    let amount = match args.num.map(Ok).or_else(|| args.next().map(u64::from_str)) {
        Some(Ok(amount)) => {
            if !(1..100).contains(&amount) {
                let content = "First argument must be an integer between 1 and 99";
                msg.error(&ctx, content).await?;

                return Ok(());
            }

            amount + 1
        }
        None | Some(Err(_)) => 2,
    };

    prune(ctx, msg.into(), amount).await
}

async fn prune(ctx: Arc<Context>, orig: CommandOrigin<'_>, amount: u64) -> Result<()> {
    let channel_id = orig.channel_id();
    let slash = matches!(orig, CommandOrigin::Interaction { .. });

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
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get channel messages");

            return Err(report);
        }
    };

    if messages.len() < 2 {
        if let Some(msg_id) = messages.pop() {
            ctx.http
                .delete_message(channel_id, msg_id)
                .exec()
                .await
                .wrap_err("failed to delete message")?;
        }

        return Ok(());
    }

    if let Err(err) = ctx.http.delete_messages(channel_id, &messages).exec().await {
        if matches!(err.kind(), ErrorType::Response {
            error: ApiError::General(err),
            ..
        } if err.code == MESSAGE_TOO_OLD_TO_BULK_DELETE)
        {
            let content = "Cannot delete messages that are older than two weeks \\:(";

            return orig.error(&ctx, content).await;
        } else {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to delete messages");

            return Err(report);
        }
    }

    let content = format!("Deleted the last {} messages", amount - 1 + slash as u64);
    let builder = MessageBuilder::new().content(content);
    let response = orig.create_message(&ctx, &builder).await?.model().await?;
    time::sleep(Duration::from_secs(6)).await;

    response
        .delete(&ctx)
        .await
        .wrap_err("failed to delete response")?;

    Ok(())
}

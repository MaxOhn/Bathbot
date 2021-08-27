use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;
use twilight_model::gateway::presence::{ActivityType, Status};

#[command]
#[short_desc("Modify the game that the bot is playing")]
#[usage("[string for new game]")]
#[owner()]
async fn changegame(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, args, num } => {
            let game = args.rest().to_owned();
            let data = CommandData::Message { msg, args, num };

            _changegame(ctx, data, game).await
        }
        CommandData::Interaction { command } => super::slash_owner(ctx, *command).await,
    }
}

pub(super) async fn _changegame(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    game: String,
) -> BotResult<()> {
    let activity_fut = ctx.set_cluster_activity(Status::Online, ActivityType::Playing, game);

    match activity_fut.await {
        Ok(_) => {
            let content = "Successfully changed game";
            let builder = MessageBuilder::new().embed(content);
            data.create_message(&ctx, builder).await?;

            Ok(())
        }
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}

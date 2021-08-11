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
    let (msg, args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

    let game = args.rest();
    let activity_fut = ctx.set_cluster_activity(Status::Online, ActivityType::Playing, game);

    match activity_fut.await {
        Ok(_) => {
            let content = "Successfully changed game";
            let builder = MessageBuilder::new().embed(content);
            msg.create_message(&ctx, builder).await?;

            Ok(())
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}

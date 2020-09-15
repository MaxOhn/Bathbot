use crate::{
    arguments::Args,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use twilight_model::{
    channel::Message,
    gateway::presence::{ActivityType, Status},
};

#[command]
#[short_desc("Modify the game that the bot is playing")]
#[usage("[string for new game]")]
#[owner()]
async fn changegame(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let game = args.rest().to_owned();
    let activity_fut = ctx.set_cluster_activity(Status::Online, ActivityType::Playing, game);
    match activity_fut.await {
        Ok(_) => {
            let content = "Successfully changed game";
            msg.respond(&ctx, content).await
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            Err(why)
        }
    }
}

use crate::{util::MessageExt, Args, BotResult, Context};

use rand::Rng;
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Get a random number")]
#[long_desc(
    "Get a random number.\n\
    If no upper limit is specified, it defaults to 100."
)]
#[usage("[upper limit]")]
async fn roll(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let num = {
        let upper_limit: u64 = args.next().and_then(|arg| arg.parse().ok()).unwrap_or(100);

        rand::thread_rng().gen_range(1..(upper_limit + 1).max(2))
    };

    let content = format!(
        "<@{}> rolls {} point{}",
        msg.author.id,
        num,
        if num == 1 { "" } else { "s" }
    );

    msg.send_response(&ctx, content).await?;

    Ok(())
}

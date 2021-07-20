use super::request_user;
use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, RatioEmbed},
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::channel::Message;

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
async fn ratios(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the user and their top scores
    let user_fut = request_user(&ctx, &name, Some(GameMode::MNA));

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

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, GameMode::MNA, &mut scores, Some(&user)).await;

    // Accumulate all necessary data
    let data = RatioEmbed::new(user, scores);

    // Creating the embed
    msg.build_response(&ctx, |m| {
        let content = format!("Average ratios of `{}`'s top 100 in mania:", name);

        m.content(content)?.embed(data.into_builder().build())
    })
    .await?;

    Ok(())
}

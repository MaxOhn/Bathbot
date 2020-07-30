use super::require_link;
use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, RatioEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::{
    backend::BestRequest,
    models::{GameMode},
};
use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Calculate the average ratios of a user's top100")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("ratio")]
async fn ratios(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return require_link(&ctx, msg).await,
    };

    // Retrieve the user and their top scores
    let join_result = tokio::try_join!(
        ctx.osu_user(&name, GameMode::MNA),
        BestRequest::with_username(&name)
            .mode(GameMode::MNA)
            .limit(100)
            .queue(ctx.osu())
    );
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.respond(&ctx, content).await;
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let data = match RatioEmbed::new(user, scores, &ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why);
        }
    };

    // Creating the embed
    let embed = data.build().build();
    msg.build_response(&ctx, |m| {
        let content = format!("Average ratios of `{}`'s top 100 in mania:", name);
        m.content(content)?.embed(embed)
    })
    .await?;
    Ok(())
}

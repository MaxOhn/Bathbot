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
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
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
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        ctx,
                        "Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(GameMode::MNA);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let user = match user_req.queue_single(&osu).await {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    let content = format!("User `{}` was not found", name);
                    msg.respond(&ctx, content).await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        let scores = match user.get_top_scores(&osu, 100, GameMode::MNA).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        (user, scores)
    };

    // Accumulate all necessary data
    let data = match RatioEmbed::new(user, scores, &ctx.data).await {
        Ok(data) => data,
        Err(why) => {
            msg.respond(ctx, GENERAL_ISSUE).await?;
            return Err(why);
        }
    };

    // Creating the embed
    msg.channel_id
        .send_message(ctx, |m| {
            let content = format!("Average ratios of `{}`'s top 100 in mania:", name);
            m.content(content).embed(|e| data.build(e))
        })
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}

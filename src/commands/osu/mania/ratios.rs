use crate::{
    arguments::NameArgs,
    embeds::{EmbedData, RatioEmbed},
    util::{globals::OSU_API_ISSUE, MessageExt},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[description = "Calculate the average ratios of a mania user's top 100"]
#[usage = "[username]"]
#[example = "badewanne3"]
#[aliases("ratio")]
async fn ratios(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
                    msg.channel_id
                        .say(ctx, format!("User `{}` was not found", name))
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let scores = match user.get_top_scores(&osu, 100, GameMode::MNA).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
    };

    // Accumulate all necessary data
    let data = match RatioEmbed::new(user, scores, &ctx.data).await {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(ctx, "Some issue while calculating ratio data, blame bade")
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Err(CommandError::from(why.to_string()));
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

use crate::{
    arguments::{Args, NameFloatArgs},
    embeds::{EmbedData, PPMissingEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
};
use std::sync::Arc;
use twilight::model::channel::Message;

async fn pp_send(mode: GameMode, ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = match NameFloatArgs::new(Args::new(msg.content.clone())) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.respond(&ctx, err_msg).await?;
            return Ok(());
        }
    };
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
    let pp = args.float;
    if pp < 0.0 {
        msg.respond(&ctx, "The pp number must be non-negative")
            .await?;
        return Ok(());
    }

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
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
        let scores = match user.get_top_scores(&osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        (user, scores)
    };

    // Accumulate all necessary data
    let data = PPMissingEmbed::new(user, scores, pp);

    // Creating the embed
    msg.channel_id
        .send_message(ctx, |m| m.embed(|e| data.build(e)))
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
pub async fn pp(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a mania user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[aliases("ppm")]
pub async fn ppmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a taiko user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[aliases("ppt")]
pub async fn pptaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp are missing to reach the given amount?")]
#[long_desc(
    "Calculate what score a ctb user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[aliases("ppc")]
pub async fn ppctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    pp_send(GameMode::CTB, ctx, msg, args).await
}

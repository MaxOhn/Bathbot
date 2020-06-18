use crate::{
    arguments::NameFloatArgs,
    embeds::{EmbedData, PPMissingEmbed},
    util::{globals::OSU_API_ISSUE, MessageExt},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

async fn pp_send(mode: GameMode, ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = match NameFloatArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.channel_id
                .say(ctx, err_msg)
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
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
        msg.channel_id
            .say(ctx, "The pp number must be non-negative")
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
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
                return Err(why.to_string().into());
            }
        };
        let scores = match user.get_top_scores(&osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
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
#[description = "Calculate what score a user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
pub async fn pp(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[description = "Calculate what score a mania user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
#[aliases("ppm")]
pub async fn ppmania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[description = "Calculate what score a taiko user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
#[aliases("ppt")]
pub async fn pptaiko(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[description = "Calculate what score a ctb user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
#[aliases("ppc")]
pub async fn ppctb(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::CTB, ctx, msg, args).await
}

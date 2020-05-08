use crate::{
    arguments::NameFloatArgs,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
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

async fn pp_send(mode: GameMode, ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = match NameFloatArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            let response = msg.channel_id.say(&ctx.http, err_msg).await?;
            discord::reaction_deletion(ctx, response, msg.author.id).await;
            return Ok(());
        }
    };
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "Either specify an osu name or link your discord \
                     to an osu profile via `<link osuname`",
                    )
                    .await?;
                return Ok(());
            }
        }
    };
    let pp = args.float;
    if pp < 0.0 {
        msg.channel_id
            .say(&ctx.http, "The pp number must be non-negative")
            .await?;
        return Ok(());
    }

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user = match user_req.queue_single(&osu).await {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))
                        .await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let scores = match user.get_top_scores(&osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
    };

    // Accumulate all necessary data
    let data = BasicEmbedData::create_ppmissing(user, scores, pp);

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    discord::reaction_deletion(&ctx, response, msg.author.id).await;
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

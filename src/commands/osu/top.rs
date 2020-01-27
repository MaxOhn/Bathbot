use crate::{
    messages::{BotEmbed, EmbedType},
    util::globals::OSU_API_ISSUE,
    Osu,
};

use rosu::{
    backend::requests::{OsuArgs, OsuRequest, UserArgs, UserBestArgs},
    models::{GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;

fn top_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let name: String = args.single_quoted()?;
    let user_args = UserArgs::with_username(&name).mode(mode);
    let best_args = UserBestArgs::with_username(&name).mode(mode).limit(100);
    let (user_req, best_req): (OsuRequest<User>, OsuRequest<Score>) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user_req = osu.create_request(OsuArgs::Users(user_args));
        let best_req = osu.create_request(OsuArgs::Best(best_args));
        (user_req, best_req)
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the user and its top scores
    let res = rt.block_on(async {
        let users = user_req
            .queue()
            .await
            .or_else(|e| Err(CommandError(format!("Error while retrieving Users: {}", e))))?;
        let scores = best_req.queue().await.or_else(|e| {
            Err(CommandError(format!(
                "Error while retrieving UserBest: {}",
                e
            )))
        })?;
        Ok((users, scores))
    });
    let (user, scores): (User, Vec<Score>) = match res {
        Ok((mut users, scores)) => {
            let user = match users.pop() {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User {} was not found", name))?;
                    return Ok(());
                }
            };
            let scores = scores[..5].to_vec();
            (user, scores)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Retrieving each score's beatmap
    let res = rt.block_on(async move {
        let mut tuples = Vec::with_capacity(100);
        for score in scores.into_iter() {
            let map = score
                .beatmap
                .as_ref()
                .unwrap()
                .get(mode)
                .await
                .or_else(|e| {
                    Err(CommandError(format!(
                        "Error while retrieving LazilyLoaded<Beatmap> of best score: {}",
                        e
                    )))
                })?;
            tuples.push((score, map));
        }
        Ok(tuples)
    });
    let score_maps = match res {
        Ok(tuple) => tuple,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Creating the embed
    let embed = BotEmbed::new(
        ctx.cache.clone(),
        mode,
        EmbedType::UserMapMulti(Box::new(user), score_maps, None),
    );
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| embed.create(e)));
    Ok(())
}

#[command]
#[description = "Display a user's top plays"]
#[usage = "badewanne3"]
#[aliases("topscores", "osutop")]
pub fn top(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display a user's top mania plays"]
#[usage = "badewanne3"]
#[aliases("topm")]
pub fn topmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display a user's top taiko plays"]
#[usage = "badewanne3"]
#[aliases("topt")]
pub fn toptaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display a user's top ctb plays"]
#[usage = "badewanne3"]
#[aliases("topc")]
pub fn topctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    top_send(GameMode::CTB, ctx, msg, args)
}

use crate::{
    messages::{BotEmbed, WhatIfPPData},
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

fn whatif_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let name: String = args.single_quoted()?;
    let pp: f32 = args.single()?;
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
            (user, scores)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let data = WhatIfPPData::new(user, scores, mode, pp);

    // Creating the embed
    let embed = BotEmbed::WhatIfPP(data);
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| embed.create(e)));
    Ok(())
}

#[command]
#[description = "Calculate the gain in pp if the user were to get a score with the given pp value"]
#[usage = "badewanne3 321.98"]
#[aliases("wi")]
pub fn whatif(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    whatif_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Calculate the gain in pp if the mania user were to get a score with the given pp value"]
#[usage = "badewanne3 321.98"]
#[aliases("wim")]
pub fn whatifmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    whatif_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Calculate the gain in pp if the taiko user were to get a score with the given pp value"]
#[usage = "badewanne3 321.98"]
#[aliases("wit")]
pub fn whatiftaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    whatif_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Calculate the gain in pp if the ctb user were to get a score with the given pp value"]
#[usage = "badewanne3 321.98"]
#[aliases("wic")]
pub fn whatifctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    whatif_send(GameMode::CTB, ctx, msg, args)
}

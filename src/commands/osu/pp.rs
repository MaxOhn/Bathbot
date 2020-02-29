use crate::{
    messages::BasicEmbedData,
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
use tokio::runtime::Runtime;

fn pp_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the name
    let name: String = match args.len() {
        0 => {
            msg.channel_id.say(
                &ctx.http,
                "You need to provide a decimal number as argument",
            )?;
            return Ok(());
        }
        1 => {
            let data = ctx.data.read();
            let links = data
                .get::<DiscordLinks>()
                .expect("Could not get DiscordLinks");
            match links.get(msg.author.id.as_u64()) {
                Some(name) => name.clone(),
                None => {
                    msg.channel_id.say(
                        &ctx.http,
                        "Either specify an osu name or link your discord \
                         to an osu profile via `<link osuname`",
                    )?;
                    return Ok(());
                }
            }
        }
        _ => args.single_quoted()?,
    };

    // Parse the pp
    let pp = match args.single::<f32>() {
        Ok(val) => val,
        Err(_) => {
            msg.channel_id.say(
                &ctx.http,
                "If no osu name is provided, the first argument must be a decimal number. \
                 If you want to give an osu name, do so as first argument. \
                 The second argument should then be the decimal number",
            )?;
            return Ok(());
        }
    };
    if pp < 0.0 {
        msg.channel_id
            .say(&ctx.http, "The pp number must be non-negative")?;
        return Ok(());
    }

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let mut rt = Runtime::new().unwrap();
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let user = match rt.block_on(user_req.queue_single(&osu)) {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let scores = match rt.block_on(user.get_top_scores(&osu, 100, mode)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
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
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
#[description = "Calculate what score a user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
pub fn pp(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Calculate what score a mania user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
#[aliases("ppm")]
pub fn ppmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Calculate what score a taiko user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
#[aliases("ppt")]
pub fn pptaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Calculate what score a ctb user is missing to \
                 reach the given total pp amount"]
#[usage = "[username] [number]"]
#[example = "badewanne3 8000"]
#[aliases("ppc")]
pub fn ppctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    pp_send(GameMode::CTB, ctx, msg, args)
}

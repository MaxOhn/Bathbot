use crate::{
    messages::BasicEmbedData, scraper::Scraper, util::globals::OSU_API_ISSUE, DiscordLinks, Osu,
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
use std::str::FromStr;
use tokio::runtime::Runtime;

fn rank_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the name
    let name: String = match args.len() {
        0 => {
            msg.channel_id.say(
                &ctx.http,
                "You need to provide a rank, either just as positive number \
                 or as country acronym with positive number e.g. `be10`",
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

    // Parse the rank
    let (country, rank) = match args.single::<String>() {
        Ok(mut val) => {
            if let Ok(num) = usize::from_str(&val) {
                (None, num)
            } else if val.len() < 3 {
                msg.channel_id.say(
                    &ctx.http,
                    "Could not parse rank. Provide it either as positive number or \
                     as country acronym followed by a positive number e.g. `be10`.",
                )?;
                return Ok(());
            } else {
                let num = val.split_off(2);
                if let Ok(num) = usize::from_str(&num) {
                    (Some(val.to_uppercase()), num)
                } else {
                    msg.channel_id.say(
                        &ctx.http,
                        "Could not parse rank. Provide it either as positive number or \
                         as country acronym followed by a positive number e.g. `be10`.",
                    )?;
                    return Ok(());
                }
            }
        }
        Err(_) => {
            msg.channel_id.say(
                &ctx.http,
                "No rank argument found. Provide it either as positive number or \
                 as country acronym followed by a positive number e.g. `be10`.",
            )?;
            return Ok(());
        }
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the rank holding user
    let rank_holder_id = {
        let data = ctx.data.read();
        let scraper = data.get::<Scraper>().expect("Could not get Scraper");
        match rt.block_on(scraper.get_userid_of_rank(rank, mode, country.as_deref())) {
            Ok(rank) => rank,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let rank_holder = {
        let user_req = UserRequest::with_user_id(rank_holder_id).mode(mode);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match rt.block_on(user_req.queue_single(&osu)) {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    msg.channel_id.say(
                        &ctx.http,
                        format!("User id `{}` was not found", rank_holder_id),
                    )?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };

    // Retrieve the user (and its top scores if user has more pp than rank_holder)
    let (user, scores): (User, Option<Vec<Score>>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
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
        if user.pp_raw > rank_holder.pp_raw {
            (user, None)
        } else {
            let scores = match rt.block_on(user.get_top_scores(&osu, 100, mode)) {
                Ok(scores) => scores,
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                    return Err(CommandError::from(why.to_string()));
                }
            };
            (user, Some(scores))
        }
    };

    // Accumulate all necessary data
    let data = BasicEmbedData::create_rank(user, scores, rank, country, rank_holder);

    // Creating the embed
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)));
    Ok(())
}

#[command]
#[description = "Calculate how many more pp a player requires to reach a given rank"]
#[usage = "badewanne3 be10"]
pub fn rank(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Calculate how many more pp a mania player requires to reach a given rank"]
#[usage = "badewanne3 be10"]
#[aliases("rankm")]
pub fn rankmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Calculate how many more pp a taiko player requires to reach a given rank"]
#[usage = "badewanne3 be10"]
#[aliases("rankt")]
pub fn ranktaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Calculate how many more pp a ctb player requires to reach a given rank"]
#[usage = "badewanne3 be10"]
#[aliases("rankc")]
pub fn rankctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::CTB, ctx, msg, args)
}

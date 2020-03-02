use crate::{
    arguments::RankArgs,
    embeds::BasicEmbedData,
    scraper::Scraper,
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

fn rank_send(mode: GameMode, ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = match RankArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.channel_id.say(&ctx.http, err_msg)?;
            return Ok(());
        }
    };
    let name = if let Some(name) = args.name {
        name
    } else {
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
    };
    let country = args.country;
    let rank = args.rank;
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
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
#[description = "Calculate how many more pp a player requires to \
                 reach a given rank"]
#[usage = "[username] [[country]number]"]
#[example = "badewanne3 be50"]
#[example = "badewanne3 123"]
#[aliases("reach")]
pub fn rank(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Calculate how many more pp a mania player requires to \
                 reach a given rank"]
#[example = "badewanne3 be50"]
#[example = "badewanne3 123"]
#[aliases("rankm", "reachmania", "reachm")]
pub fn rankmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Calculate how many more pp a taiko player requires to \
                 reach a given rank"]
#[example = "badewanne3 be50"]
#[example = "badewanne3 123"]
#[aliases("rankt", "reachtaiko", "reacht")]
pub fn ranktaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Calculate how many more pp a ctb player requires to \
                 reach a given rank"]
#[example = "badewanne3 be50"]
#[example = "badewanne3 123"]
#[aliases("rankc", "reachctb", "reachc")]
pub fn rankctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::CTB, ctx, msg, args)
}

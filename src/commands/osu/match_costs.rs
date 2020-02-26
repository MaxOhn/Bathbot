use crate::{commands::arguments, messages::BasicEmbedData, util::globals::OSU_API_ISSUE, Osu};

use rosu::backend::requests::{MatchRequest, UserRequest};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;

#[command]
#[description = "Calculate a performance rating for each player in the multiplayer match"]
#[usage = "58320988 0"]
#[aliases("mc", "matchcost")]
fn matchcosts(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the match id
    let match_id = if let Some(match_id) = arguments::get_regex_id(&args.single::<String>()?) {
        match_id
    } else {
        msg.channel_id.say(
            &ctx.http,
            "The first argument must be either a match id or the multiplayer link to a match",
        )?;
        return Ok(());
    };
    // Parse amount of warmups
    let warmups = args.single::<usize>().unwrap_or(2);

    let mut rt = Runtime::new().unwrap();

    // Retrieve the match
    let osu_match = {
        let match_req = MatchRequest::with_match_id(match_id);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match rt.block_on(match_req.queue_single(&osu)) {
            Ok(osu_match) => osu_match,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };

    // Retrieve all usernames of the match
    let users: HashMap<u32, String> = {
        let mut users = HashMap::new();
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for game in osu_match.games.iter() {
            #[allow(clippy::map_entry)]
            for score in game.scores.iter() {
                if !users.contains_key(&score.user_id) {
                    let req = UserRequest::with_user_id(score.user_id);
                    let name = match rt.block_on(req.queue_single(&osu)) {
                        Ok(result) => match result {
                            Some(user) => user.username,
                            None => score.user_id.to_string(),
                        },
                        Err(why) => {
                            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                            return Err(CommandError::from(why.to_string()));
                        }
                    };
                    users.insert(score.user_id, name);
                }
            }
        }
        users
    };

    // Accumulate all necessary data
    let data = BasicEmbedData::create_match_costs(users, osu_match, warmups);

    // Creating the embed
    msg.channel_id.send_message(&ctx.http, |m| {
        if warmups > 0 {
            let mut content = String::from("Ignoring the first ");
            if warmups == 1 {
                content.push_str("map");
            } else {
                content.push_str(&format!("{} maps", warmups));
            }
            content.push_str(" as warmup:");
            m.content(content);
        }
        m.embed(|e| data.build(e))
    })?;
    Ok(())
}

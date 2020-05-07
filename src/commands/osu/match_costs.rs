use crate::{
    arguments::MatchArgs,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    Osu,
};

use rosu::backend::requests::{MatchRequest, UserRequest};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{collections::HashMap, fmt::Write};

#[command]
#[description = "Calculate a performance rating for each player \
                 in the given multiplayer match. The optional second \
                 argument is the amount of played warmups, defaults to 2.\n\
                 More info over at https://github.com/dain98/Minccino#faq"]
#[usage = "[match url / match id] [amount of warmups]"]
#[example = "58320988 1"]
#[example = "https://osu.ppy.sh/community/matches/58320988"]
#[aliases("mc", "matchcost")]
async fn matchcosts(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = match MatchArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.channel_id.say(&ctx.http, err_msg).await?;
            return Ok(());
        }
    };
    let match_id = args.match_id;
    let warmups = args.warmups;

    // Retrieve the match
    let osu_match = {
        let match_req = MatchRequest::with_match_id(match_id);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match match_req.queue_single(&osu).await {
            Ok(osu_match) => osu_match,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };

    // Retrieve all usernames of the match
    let users: HashMap<u32, String> = {
        let mut users = HashMap::new();
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().expect("Could not get osu client");
        for game in osu_match.games.iter() {
            #[allow(clippy::map_entry)]
            for score in game.scores.iter() {
                if !users.contains_key(&score.user_id) {
                    let req = UserRequest::with_user_id(score.user_id);
                    let name = match req.queue_single(&osu).await {
                        Ok(result) => match result {
                            Some(user) => user.username,
                            None => score.user_id.to_string(),
                        },
                        Err(why) => {
                            msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
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
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            if warmups > 0 {
                let mut content = String::from("Ignoring the first ");
                if warmups == 1 {
                    content.push_str("map");
                } else {
                    let _ = write!(content, "{} maps", warmups);
                }
                content.push_str(" as warmup:");
                m.content(content);
            }
            m.embed(|e| data.build(e))
        })
        .await?;

    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}

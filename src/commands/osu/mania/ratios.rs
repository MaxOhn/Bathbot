use crate::{messages::BasicEmbedData, util::globals::OSU_API_ISSUE, DiscordLinks, Osu};

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

#[command]
#[description = "Calculate the average ratios of a mania user's top 100"]
#[usage = "badewanne3"]
#[aliases("ratio")]
pub fn ratios(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the name
    let name: String = if args.is_empty() {
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
    } else {
        args.single_quoted()?
    };

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(GameMode::MNA);
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
        let scores = match rt.block_on(user.get_top_scores(&osu, 100, GameMode::MNA)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, scores)
    };

    // Accumulate all necessary data
    let data = match BasicEmbedData::create_ratio(user, scores) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating ratio data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Creating the embed
    let _ = msg.channel_id.send_message(&ctx.http, |m| {
        let content = format!("Average ratios of `{}`'s top 100 in mania:", name);
        m.content(content).embed(|e| data.build(e))
    });
    Ok(())
}

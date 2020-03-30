use crate::{
    arguments::NameArgs,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    DiscordLinks, Osu, Scraper,
};

use rosu::backend::requests::UserRequest;
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;

#[command]
#[description = "Display the 10 most played maps of a user"]
#[usage = "[username]"]
#[example = "badewanne3"]
fn mostplayed(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameArgs::new(args);
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

    // Retrieve the user
    let (user, maps) = {
        let user_req = UserRequest::with_username(&name);
        let mut rt = Runtime::new().unwrap();
        let data = ctx.data.read();
        let user = {
            let osu = data.get::<Osu>().expect("Could not get osu client");
            match rt.block_on(user_req.queue_single(&osu)) {
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
            }
        };
        let maps = {
            let scraper = data.get::<Scraper>().expect("Could not get Scraper");
            match rt.block_on(scraper.get_most_played(user.user_id, 10)) {
                Ok(maps) => maps,
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                    return Err(CommandError::from(why.to_string()));
                }
            }
        };
        (user, maps)
    };

    // Accumulate all necessary data
    let data = BasicEmbedData::create_mostplayed(user, maps);

    // Creating the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

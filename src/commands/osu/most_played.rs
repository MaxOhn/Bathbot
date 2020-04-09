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

#[command]
#[description = "Display the 10 most played maps of a user"]
#[usage = "[username]"]
#[example = "badewanne3"]
async fn mostplayed(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameArgs::new(args);
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

    // Retrieve the user
    let (user, maps) = {
        let user_req = UserRequest::with_username(&name);
        let data = ctx.data.read().await;
        let user = {
            let osu = data.get::<Osu>().expect("Could not get osu client");
            match user_req.queue_single(&osu).await {
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
            }
        };
        let maps = {
            let scraper = data.get::<Scraper>().expect("Could not get Scraper");
            match scraper.get_most_played(user.user_id, 10).await {
                Ok(maps) => maps,
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
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
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    discord::reaction_deletion(&ctx, response, msg.author.id);
    Ok(())
}

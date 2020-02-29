use crate::{
    database::MySQL,
    util::{discord, globals::DATABASE_ISSUE},
    DiscordLinks,
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[description = "Link your discord account to an osu name. \
                 If no arguments are provided, I will unlink \
                 your discord account from any osu name."]
#[usage = "[username]"]
#[example = "badewanne3"]
fn link(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let id = *msg.author.id.as_u64();
    if args.is_empty() {
        {
            let mut data = ctx.data.write();
            let links = data
                .get_mut::<DiscordLinks>()
                .expect("Could not get DiscordLinks");
            links.remove_entry(&id);
        }
        {
            let data = ctx.data.read();
            let mysql = data.get::<MySQL>().expect("Could not get MySQL");
            if let Err(why) = mysql.remove_discord_link(id) {
                msg.channel_id.say(&ctx.http, DATABASE_ISSUE)?;
                return Err(CommandError(format!(
                    "Error while removing discord link from database: {}",
                    why
                )));
            }
        }
        msg.channel_id.say(&ctx.http, "You are no longer linked")?;
        Ok(())
    } else {
        let name = args.single_quoted::<String>()?;
        {
            let mut data = ctx.data.write();
            let links = data
                .get_mut::<DiscordLinks>()
                .expect("Could not get DiscordLinks");
            let value = links.entry(id).or_insert_with(String::default);
            *value = name.clone();
        }
        {
            let data = ctx.data.read();
            let mysql = data.get::<MySQL>().expect("Could not get MySQL");
            if let Err(why) = mysql.add_discord_link(id, &name) {
                msg.channel_id.say(&ctx.http, DATABASE_ISSUE)?;
                return Err(CommandError(format!(
                    "Error while adding discord link to database: {}",
                    why
                )));
            }
        }
        let response = msg.channel_id.say(
            &ctx.http,
            format!(
                "I linked discord's `{}` with osu's `{}`",
                msg.author.name, name
            ),
        )?;

        // Save the response owner
        discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
        Ok(())
    }
}

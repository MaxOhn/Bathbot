use crate::{
    database::MySQL,
    util::{discord, globals::GENERAL_ISSUE},
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
async fn link(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let id = *msg.author.id.as_u64();
    if args.is_empty() {
        {
            let mut data = ctx.data.write().await;
            let links = data.get_mut::<DiscordLinks>().unwrap();
            links.remove_entry(&id);
        }
        {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            if let Err(why) = mysql.remove_discord_link(id) {
                msg.channel_id.say(&ctx.http, GENERAL_ISSUE).await?;
                return Err(CommandError(format!(
                    "Error while removing discord link from DB: {}",
                    why
                )));
            }
        }
        msg.channel_id
            .say(&ctx.http, "You are no longer linked")
            .await?;
        Ok(())
    } else {
        let name = args.single_quoted::<String>()?;
        {
            let mut data = ctx.data.write().await;
            let links = data.get_mut::<DiscordLinks>().unwrap();
            let value = links.entry(id).or_insert_with(String::default);
            *value = name.clone();
        }
        {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            match mysql.add_discord_link(id, &name) {
                Ok(_) => debug!("Discord user {} now linked to osu name {} in DB", id, name),
                Err(why) => {
                    msg.channel_id.say(&ctx.http, GENERAL_ISSUE).await?;
                    return Err(CommandError(format!(
                        "Error while adding discord link to DB: {}",
                        why
                    )));
                }
            }
        }
        let response = msg
            .channel_id
            .say(
                &ctx.http,
                format!(
                    "I linked discord's `{}` with osu's `{}`",
                    msg.author.name, name
                ),
            )
            .await?;

        discord::reaction_deletion(&ctx, response, msg.author.id).await;
        Ok(())
    }
}

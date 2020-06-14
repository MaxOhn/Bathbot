use crate::{commands::checks::*, util::MessageExt, BgVerified, MySQL};

use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[owners_only]
#[checks(BgVerifiedCheck)]
#[description = "Reload bg verified users from database"]
async fn reloadverified(ctx: &Context, msg: &Message) -> CommandResult {
    let mut data = ctx.data.write().await;
    let verified_users = {
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_bg_verified() {
            Ok(users) => users,
            Err(why) => {
                msg.channel_id
                    .say(ctx, "Error while retrieving verified users from database")
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
        }
    };
    *data.get_mut::<BgVerified>().unwrap() = verified_users;
    msg.channel_id
        .say(ctx, "Reload successful")
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}

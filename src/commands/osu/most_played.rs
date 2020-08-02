use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, MostPlayedEmbed},
    pagination::{MostPlayedPagination, Pagination},
    util::{constants::OSU_API_ISSUE, numbers, MessageExt},
    BotResult, Context,
};

use rosu::models::GameMode;
use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Display the most played maps of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mp")]
async fn mostplayed(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the user
    let user = match ctx.osu_user(&name, GameMode::STD).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let maps = match ctx.clients.custom.get_most_played(user.user_id, 50).await {
        Ok(maps) => maps,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(10, maps.len());
    let data = MostPlayedEmbed::new(&user, maps.iter().take(10), (1, pages));

    // Creating the embed
    let embed = data.build().build();
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = MostPlayedPagination::new(response, user, maps);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

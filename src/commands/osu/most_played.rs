use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, MostPlayedEmbed},
    pagination::{MostPlayedPagination, Pagination},
    util::{constants::OSU_API_ISSUE, numbers, MessageExt},
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::channel::Message;

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

    // Retrieve the user and their most played maps
    let user_fut = ctx.osu().user(&name).mode(GameMode::STD);
    let maps_fut_1 = ctx.osu().user_most_played(&name).limit(50);
    let maps_fut_2 = ctx.osu().user_most_played(&name).limit(50).offset(50);

    let (user, maps) = match tokio::try_join!(user_fut, maps_fut_1, maps_fut_2) {
        Ok((user, mut maps, mut maps_2)) => {
            maps.append(&mut maps_2);

            (user, maps)
        }
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(10, maps.len());
    let data = MostPlayedEmbed::new(&user, maps.iter().take(10), (1, pages));

    // Creating the embed
    let embed = data.build().build()?;
    let response = msg.respond_embed(&ctx, embed).await?;

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
            unwind_error!(warn, why, "Pagination error (mostplayed): {}")
        }
    });

    Ok(())
}

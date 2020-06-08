use crate::{
    arguments::NameArgs,
    embeds::BasicEmbedData,
    pagination::{Pagination, ReactionData},
    util::{globals::OSU_API_ISSUE, numbers},
    DiscordLinks, Osu, Scraper,
};

use futures::StreamExt;
use rosu::backend::requests::UserRequest;
use serenity::{
    collector::ReactionAction,
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{convert::TryFrom, sync::Arc, time::Duration};

#[command]
#[description = "Display the 10 most played maps of a user"]
#[usage = "[username]"]
#[example = "badewanne3"]
async fn mostplayed(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameArgs::new(args);
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
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
            let osu = data.get::<Osu>().unwrap();
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
            let scraper = data.get::<Scraper>().unwrap();
            match scraper.get_most_played(user.user_id, 50).await {
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
    let pages = numbers::div_euclid(10, maps.len());
    let data = BasicEmbedData::create_mostplayed(&user, maps.iter().take(10), (1, pages));

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(&ctx)
        .timeout(Duration::from_secs(90))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        let reaction_type = ReactionType::try_from(reaction).unwrap();
        response.react(&ctx.http, reaction_type).await?;
    }

    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    tokio::spawn(async move {
        let mut pagination = Pagination::most_played(user, maps);
        while let Some(reaction) = collector.next().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction) = &reaction.emoji {
                    match pagination.next_reaction(reaction.as_str()).await {
                        Ok(data) => match data {
                            ReactionData::Delete => response.delete((&cache, &*http)).await?,
                            ReactionData::None => {}
                            _ => {
                                response
                                    .edit((&cache, &*http), |m| m.embed(|e| data.build(e)))
                                    .await?
                            }
                        },
                        Err(why) => warn!("Error while using paginator for mostplayed: {}", why),
                    }
                }
            }
        }
        for &reaction in reactions.iter() {
            let reaction_type = ReactionType::try_from(reaction).unwrap();
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction_type)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

use crate::{
    arguments::NameArgs,
    embeds::BasicEmbedData,
    scraper::MostPlayedMap,
    util::{globals::OSU_API_ISSUE, numbers},
    DiscordLinks, Osu, Scraper,
};

use rosu::{backend::requests::UserRequest, models::User};
use serenity::{
    collector::{ReactionAction, ReactionCollectorBuilder},
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{sync::Arc, time::Duration};

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
    let mut collector = ReactionCollectorBuilder::new(&ctx)
        .author_id(msg.author.id)
        .message_id(response.id)
        .timeout(Duration::from_secs(60))
        .await;
    let mut idx = 0;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        response.react(&ctx.http, reaction).await?;
    }

    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    tokio::spawn(async move {
        while let Some(reaction) = collector.receive_one().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction) = &reaction.emoji {
                    let reaction_data = reaction_data(reaction.as_str(), &mut idx, &user, &maps);
                    match reaction_data.await {
                        ReactionData::None => {}
                        ReactionData::Delete => response.delete((&cache, &*http)).await?,
                        ReactionData::Data(data) => {
                            response
                                .edit((&cache, &*http), |m| m.embed(|e| data.build(e)))
                                .await?
                        }
                    }
                }
            }
        }
        for &reaction in reactions.iter() {
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

enum ReactionData {
    Data(Box<BasicEmbedData>),
    Delete,
    None,
}

async fn reaction_data(
    reaction: &str,
    idx: &mut usize,
    user: &User,
    maps: &[MostPlayedMap],
) -> ReactionData {
    let amount = maps.len();
    let pages = numbers::div_euclid(10, amount);
    let data = match reaction {
        "❌" => return ReactionData::Delete,
        "⏮️" => {
            if *idx > 0 {
                *idx = 0;
                BasicEmbedData::create_mostplayed(
                    &user,
                    maps.iter().skip(*idx).take(10),
                    (*idx / 10 + 1, pages),
                )
            } else {
                return ReactionData::None;
            }
        }
        "⏪" => {
            if *idx > 0 {
                *idx = idx.saturating_sub(10);
                BasicEmbedData::create_mostplayed(
                    &user,
                    maps.iter().skip(*idx).take(10),
                    (*idx / 10 + 1, pages),
                )
            } else {
                return ReactionData::None;
            }
        }
        "⏩" => {
            let limit = if amount % 10 == 0 {
                amount - 10
            } else {
                amount - amount % 10
            };
            if *idx < limit {
                *idx = limit.min(*idx + 10);
                BasicEmbedData::create_mostplayed(
                    &user,
                    maps.iter().skip(*idx).take(10),
                    (*idx / 10 + 1, pages),
                )
            } else {
                return ReactionData::None;
            }
        }
        "⏭️" => {
            let limit = if amount % 10 == 0 {
                amount - 10
            } else {
                amount - amount % 10
            };
            if *idx < limit {
                *idx = limit;
                BasicEmbedData::create_mostplayed(
                    &user,
                    maps.iter().skip(*idx).take(10),
                    (*idx / 10 + 1, pages),
                )
            } else {
                return ReactionData::None;
            }
        }
        _ => return ReactionData::None,
    };
    ReactionData::Data(Box::new(data))
}

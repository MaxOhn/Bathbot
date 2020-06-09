mod game;
mod hints;
mod img_reveal;
mod util;

pub use game::BackGroundGame;
use hints::Hints;
use img_reveal::ImageReveal;

use crate::{
    embeds::BasicEmbedData,
    pagination::{Pagination, ReactionData},
    util::{discord, numbers},
    BgGames, Error, MySQL,
};

use rosu::models::GameMode;
use serenity::{
    collector::{MessageCollectorBuilder, ReactionAction},
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::{Message, ReactionType},
        id::UserId,
    },
    prelude::Context,
};
use std::{collections::HashMap, convert::TryFrom, sync::Arc, time::Duration};
use tokio::stream::StreamExt;

#[command]
#[description = "Given part of a map's background, try to guess \
the **title** of the map's song.\nCheck `<bg` for more help"]
#[aliases("bg")]
#[sub_commands("start", "hint", "bigger", "stop", "stats", "ranking")]
async fn backgroundgame(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let response = if args.is_empty() {
        let data = BasicEmbedData::create_bg_help();
        msg.channel_id
            .send_message(ctx, |m| m.embed(|e| data.build(e)))
            .await?
    } else {
        msg.channel_id
            .say(
                ctx,
                "That's not a valid subcommand. Check `<bg` for more help.",
            )
            .await?
    };
    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}

#[command]
#[aliases("s", "skip", "resolve", "r")]
#[sub_commands("mania")]
async fn start(ctx: &Context, msg: &Message) -> CommandResult {
    _start(GameMode::STD, ctx, msg).await
}

#[command]
#[aliases("m")]
async fn mania(ctx: &Context, msg: &Message) -> CommandResult {
    _start(GameMode::MNA, ctx, msg).await
}

#[allow(clippy::map_entry)]
async fn _start(mode: GameMode, ctx: &Context, msg: &Message) -> CommandResult {
    let channel = msg.channel_id;
    let mut data = ctx.data.write().await;
    let games = data.get_mut::<BgGames>().unwrap();
    if !games.contains_key(&channel) {
        let game = BackGroundGame::new(mode).await;
        let collector = MessageCollectorBuilder::new(&ctx)
            .channel_id(channel)
            .filter(|msg| !msg.author.bot)
            .await;
        game.start(
            collector,
            channel,
            Arc::clone(&ctx.data),
            Arc::clone(&ctx.http),
        );
        games.insert(channel, game);
    } else {
        games.get_mut(&channel).unwrap().restart()?;
    }
    Ok(())
}

#[command]
#[aliases("h", "tip")]
#[bucket = "bg_hint"]
async fn hint(ctx: &Context, msg: &Message) -> CommandResult {
    let hint = {
        let mut data = ctx.data.write().await;
        let game = data
            .get_mut::<BgGames>()
            .and_then(|games| games.get(&msg.channel_id));
        if let Some(game) = game {
            Some(game.hint().await)
        } else {
            None
        }
    };
    let _ = if let Some(hint) = hint {
        msg.channel_id.say(&ctx.http, hint).await?
    } else {
        msg.channel_id
            .say(
                &ctx.http,
                "There is no running game in this channel, \
                start one with `<bg s`",
            )
            .await?
    };
    Ok(())
}

#[command]
#[aliases("b", "enhance")]
#[bucket = "bg_bigger"]
async fn bigger(ctx: &Context, msg: &Message) -> CommandResult {
    let img: Option<Result<Vec<u8>, Error>> = {
        let mut data = ctx.data.write().await;
        let game = data.get_mut::<BgGames>().unwrap().get_mut(&msg.channel_id);
        if let Some(game) = game {
            Some(game.sub_image().await)
        } else {
            None
        }
    };
    if let Some(Ok(img)) = img {
        msg.channel_id
            .send_message(&ctx.http, |m| {
                let bytes: &[u8] = &img;
                m.add_file((bytes, "bg_img.png"))
            })
            .await?;
    } else {
        msg.channel_id
            .say(
                &ctx.http,
                "There is no running game in this channel, \
                start one with `<bg s`",
            )
            .await?;
    }
    Ok(())
}

#[command]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    let channel = msg.channel_id;
    let mut data = ctx.data.write().await;
    let games = data.get_mut::<BgGames>().unwrap();
    if !games.contains_key(&channel) {
        msg.channel_id
            .say(
                &ctx.http,
                "There is no running game in this channel, \
                start one with `<bg s`",
            )
            .await?;
    } else {
        games.get_mut(&channel).unwrap().stop()?;
    }
    Ok(())
}

#[command]
async fn stats(ctx: &Context, msg: &Message) -> CommandResult {
    let score = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        mysql.get_bggame_score(msg.author.id.0).await.ok()
    };
    let response = if let Some(score) = score {
        msg.reply(
            (&ctx.cache, &*ctx.http),
            format!("You've guessed {} backgrounds correctly!", score),
        )
        .await?
    } else {
        msg.reply(
            (&ctx.cache, &*ctx.http),
            "Looks like you haven't guessed any backgrounds yet".to_string(),
        )
        .await?
    };
    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}

#[command]
#[aliases("leaderboard", "lb")]
async fn ranking(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let global = msg.guild_id.is_none()
        || args
            .single::<String>()
            .map(|arg| ["g", "global"].contains(&arg.as_str()))
            .unwrap_or_else(|_| false);
    let mut scores = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        mysql.all_bggame_scores().await?
    };
    if !global && msg.guild_id.is_some() {
        let guild_id = msg.guild_id.unwrap();
        let cache = ctx.cache.read().await;
        let cache_guild = cache.guilds.get(&guild_id);
        if let Some(guild) = cache_guild {
            let guild = guild.read().await;
            let members: Vec<u64> = guild.members.keys().map(|id| id.0).collect();
            scores.retain(|(user, _)| members.iter().any(|member| member == user));
        }

        if scores.is_empty() {
            let response = msg
                .channel_id
                .say(
                    &ctx,
                    "Looks like no one on this server has played the backgroundgame yet",
                )
                .await?;
            discord::reaction_deletion(&ctx, response, msg.author.id).await;
            return Ok(());
        }
    }
    scores.sort_by(|(_, a), (_, b)| b.cmp(&a));
    let author_idx = scores.iter().position(|(user, _)| *user == msg.author.id.0);

    // Gather usernames for initial page
    let mut usernames = HashMap::with_capacity(15);
    for &id in scores.iter().take(15).map(|(id, _)| id) {
        let name = if let Ok(user) = UserId(id).to_user(ctx).await {
            user.name
        } else {
            String::from("Unknown user")
        };
        usernames.insert(id, name);
    }
    let initial_scores = scores
        .iter()
        .take(15)
        .map(|(id, score)| (usernames.get(&id).unwrap(), *score))
        .collect();

    // Prepare initial page
    let pages = numbers::div_euclid(15, scores.len());
    let data = BasicEmbedData::create_bg_ranking(author_idx, initial_scores, global, 1, (1, pages));

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    if scores.len() <= 15 {
        discord::reaction_deletion(&ctx, response, msg.author.id).await;
        return Ok(());
    }

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(&ctx)
        .timeout(Duration::from_secs(60))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "*️⃣", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        let reaction_type = ReactionType::try_from(reaction).unwrap();
        response.react(&ctx.http, reaction_type).await?;
    }
    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    tokio::spawn(async move {
        let mut pagination =
            Pagination::bg_ranking(author_idx, scores, global, Arc::clone(&http), cache.clone());
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
                        Err(why) => warn!("Error while using paginator for bg ranking: {}", why),
                    }
                }
            }
        }

        // Remove initial reactions
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

#![allow(unused_imports)]

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

use futures::StreamExt;
use rosu::models::GameMode;
use serenity::{
    collector::{MessageCollectorBuilder, ReactionAction},
    framework::standard::{macros::command, Args, CommandResult},
    http::client::Http,
    model::{
        channel::ReactionType,
        id::{ChannelId, UserId},
        prelude::Message,
    },
    prelude::{Context, RwLock as SRwLock, TypeMap},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Write,
    sync::Arc,
    time::Duration,
};

#[command]
#[description = "Play the background game!\n\
With `<bg start` you can start the game in a channel \
which makes me post a piece of some map's background. \
Then you have to guess the **title** of the map's song.\n\
With `<bg hint` I will give you some tips (repeatable).\n\
With `<bg bigger` I will increase the size of the revealed part.\n\
With `<bg resolve` I will show you the solution.\n\
With `<bg stop` I will stop the game in this channel.\n\
With `<bg stats` you can check on how many maps you guessed.\n\
With `<bg ranking [global]` you can check the (global) leaderboard for correct guesses.\n\
Subcommands can be abbreviated with `s, h, b, r`, e.g. `<bg h` for hints.\n\
With `<bg start mania` (`<bg s m`) I will give mania backgrounds to guess."]
#[aliases("bg")]
#[sub_commands("start", "hint", "bigger", "stats", "ranking")]
async fn backgroundgame(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    if !args.is_empty() {
        let arg = args.single_quoted::<String>()?;
        if !(arg.starts_with('s') || arg.starts_with('r')) {
            basic_msg(&ctx.http, msg.channel_id).await?;
        }
    } else {
        basic_msg(&ctx.http, msg.channel_id).await?;
    }
    Ok(())
}

async fn basic_msg(http: &Http, channel: ChannelId) -> CommandResult {
    channel
        .say(
            http,
            "Use `<bg s` to (re)start the game, \
            `<bg b` to increase the image size, \
            `<bg h` to get a hint, \
            `<bg stop` to stop the game, \
            `<bg stats` to check your correct guesses, \
            or `<bg ranking [global]` to check the (global) leaderboard.\n\
            For mania backgrounds use `<bg start mania` (`<bg s m`) instead of `<bg start`, \
            everything else stays the same",
        )
        .await?;
    Ok(())
}

#[command]
#[aliases("s")]
#[sub_commands("mania")]
async fn start(ctx: &mut Context, msg: &Message) -> CommandResult {
    _start(GameMode::STD, ctx, msg).await
}

#[command]
#[aliases("m")]
async fn mania(ctx: &mut Context, msg: &Message) -> CommandResult {
    _start(GameMode::MNA, ctx, msg).await
}

#[allow(clippy::map_entry)]
async fn _start(mode: GameMode, ctx: &mut Context, msg: &Message) -> CommandResult {
    let channel = msg.channel_id;
    let mut data = ctx.data.write().await;
    let games = data.get_mut::<BgGames>().expect("Could not get BgGames");
    if !games.contains_key(&channel) {
        let game = BackGroundGame::new(mode == GameMode::STD);
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
    }
    Ok(())
}

#[command]
#[aliases("h")]
#[bucket = "bg_hint"]
async fn hint(ctx: &mut Context, msg: &Message) -> CommandResult {
    let hint = {
        let mut data = ctx.data.write().await;
        let game = data
            .get_mut::<BgGames>()
            .expect("Could not get BgGames")
            .get(&msg.channel_id);
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
async fn bigger(ctx: &mut Context, msg: &Message) -> CommandResult {
    let img: Option<Result<Vec<u8>, Error>> = {
        let mut data = ctx.data.write().await;
        let game = data
            .get_mut::<BgGames>()
            .expect("Could not get BgGames")
            .get_mut(&msg.channel_id);
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
async fn stats(ctx: &mut Context, msg: &Message) -> CommandResult {
    let score = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.get_bggame_score(msg.author.id.0).ok()
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
async fn ranking(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let global = args
        .single::<String>()
        .map(|arg| ["g", "global"].contains(&arg.as_str()))
        .unwrap_or_else(|_| false);
    let mut scores = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.all_bggame_scores()?
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
        let name = if let Ok(user) = UserId(id).to_user(&ctx).await {
            user.name
        } else {
            String::from("Unknown user")
        };
        usernames.insert(id, name);
    }
    let initial_scores = scores
        .iter()
        .take(15)
        .map(|(id, score)| (usernames.remove(&id).unwrap(), *score))
        .collect();

    // Prepare initial page
    let pages = numbers::div_euclid(15, scores.len());
    let data = BasicEmbedData::create_bg_ranking(author_idx, initial_scores, 1, (1, pages));

    // Creating the embed
    let mut response = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.content(format!(
                "{} leaderboard for correct guesses:",
                if global { "Global" } else { "Server" }
            ))
            .embed(|e| data.build(e))
        })
        .await?;

    if scores.len() <= 15 {
        discord::reaction_deletion(&ctx, response, msg.author.id).await;
        return Ok(());
    }

    // Collect reactions of author on the response
    let mut collector = response
        .await_reactions(&ctx)
        .timeout(Duration::from_secs(90))
        .author_id(msg.author.id)
        .await;

    // Add initial reactions
    let reactions = ["⏮️", "⏪", "*️⃣", "⏩", "⏭️"];
    for &reaction in reactions.iter() {
        response.react(&ctx.http, reaction).await?;
    }
    // Check if the author wants to edit the response
    let http = Arc::clone(&ctx.http);
    let cache = ctx.cache.clone();
    tokio::spawn(async move {
        let mut pagination =
            Pagination::bg_ranking(author_idx, scores, Arc::clone(&http), cache.clone());
        while let Some(reaction) = collector.next().await {
            if let ReactionAction::Added(reaction) = &*reaction {
                if let ReactionType::Unicode(reaction) = &reaction.emoji {
                    match pagination.next_reaction(reaction.as_str()).await {
                        Ok(data) => match data {
                            ReactionData::Delete => response.delete((&cache, &*http)).await?,
                            ReactionData::None => {}
                            _ => {
                                response
                                    .edit((&cache, &*http), |m| {
                                        m.content(format!(
                                            "{} leaderboard for correct guesses:",
                                            if global { "Global" } else { "Server" }
                                        ))
                                        .embed(|e| data.build(e))
                                    })
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
            response
                .channel_id
                .delete_reaction(&http, response.id, None, reaction)
                .await?;
        }
        Ok::<_, serenity::Error>(())
    });
    Ok(())
}

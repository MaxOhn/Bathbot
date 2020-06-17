mod game;
mod hints;
mod img_reveal;
mod util;

pub use game::BackGroundGame;
use hints::Hints;
use img_reveal::ImageReveal;

use crate::{
    commands::utility::MapsetTags,
    database::MapsetTagWrapper,
    embeds::{BGHelpEmbed, BGRankingEmbed, BGStartEmbed, BGTagsEmbed, EmbedData},
    pagination::{BGRankingPagination, Pagination},
    util::{numbers, MessageExt},
    BgGames, Error, MySQL,
};

use rosu::models::GameMode;
use serenity::{
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
        let data = BGHelpEmbed::new();
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
    response.reaction_delete(ctx, msg.author.id).await;
    Ok(())
}

#[command]
#[aliases("s", "skip", "resolve", "r")]
#[sub_commands("mania")]
async fn start(ctx: &Context, msg: &Message) -> CommandResult {
    let channel = msg.channel_id;
    // Check if channel already has a running game
    {
        let mut data = ctx.data.write().await;
        let games = data.get_mut::<BgGames>().unwrap();
        if games.contains_key(&channel) {
            games.get_mut(&channel).unwrap().restart()?;
            return Ok(());
        }
    }
    // Send initial message
    let embed_data = BGStartEmbed::new();
    let response = channel
        .send_message(ctx, |m| m.embed(|e| embed_data.build(e)))
        .await?;
    let mut collector = response
        .await_reactions(ctx)
        .timeout(Duration::from_secs(60))
        .author_id(msg.author.id)
        .removed(true)
        .await;
    let reactions = [
        "🍋",
        "🤓",
        "🤡",
        "🎨",
        "🍨",
        "👨‍🌾",
        "😱",
        "🪀",
        "🟦",
        "🗽",
        "🌀",
        "👴",
        "💯",
        "✅",
    ];
    for &reaction in reactions.iter() {
        let reaction = ReactionType::try_from(reaction).unwrap();
        response.react(ctx, reaction).await?;
    }
    // Run collector
    let mut included = MapsetTags::empty();
    let mut excluded = MapsetTags::empty();
    while let Some(reaction) = collector.next().await {
        let tag = if let ReactionType::Unicode(ref reaction) = reaction.as_inner_ref().emoji {
            match reaction.as_str() {
                "🍋" => MapsetTags::Easy,
                "🤓" => MapsetTags::Hard,
                "🤡" => MapsetTags::Meme,
                "👴" => MapsetTags::Old,
                "😱" => MapsetTags::HardName,
                "🟦" => MapsetTags::BlueSky,
                "🪀" => MapsetTags::Alternate,
                "🗽" => MapsetTags::English,
                "👨‍🌾" => MapsetTags::Farm,
                "💯" => MapsetTags::Tech,
                "🎨" => MapsetTags::Weeb,
                "🌀" => MapsetTags::Streams,
                "🍨" => MapsetTags::Kpop,
                "✅" => break,
                _ => continue,
            }
        } else {
            continue;
        };
        if reaction.is_added() {
            included.insert(tag);
            excluded.remove(tag);
        } else {
            excluded.insert(tag);
            included.remove(tag);
        }
    }
    collector.stop();
    // Get all mapsets matching the given tags
    let mapsets = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_specific_tags_mapset(GameMode::STD, included, excluded) {
            Ok(mapsets) => mapsets,
            Err(why) => {
                channel
                    .say(ctx, "Some database issue, blame bade")
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
        }
    };
    let embed_data = BGTagsEmbed::new(included, excluded, mapsets.len());
    channel
        .send_message(ctx, |m| m.embed(|e| embed_data.build(e)))
        .await?;
    if mapsets.is_empty() {
        return Ok(());
    }
    _start(ctx, msg, mapsets).await
}

#[command]
#[aliases("m")]
async fn mania(ctx: &Context, msg: &Message) -> CommandResult {
    {
        let mut data = ctx.data.write().await;
        let games = data.get_mut::<BgGames>().unwrap();
        if games.contains_key(&msg.channel_id) {
            games.get_mut(&msg.channel_id).unwrap().restart()?;
            return Ok(());
        }
    }
    let data = ctx.data.read().await;
    let mysql = data.get::<MySQL>().unwrap();
    let mapsets = match mysql.get_all_tags_mapset(GameMode::MNA) {
        Ok(mapsets) => mapsets,
        Err(why) => {
            msg.channel_id
                .say(ctx, "Some database issue, blame bade")
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Err(why.to_string().into());
        }
    };
    _start(ctx, msg, mapsets).await
}

#[allow(clippy::map_entry)]
async fn _start(ctx: &Context, msg: &Message, mapsets: Vec<MapsetTagWrapper>) -> CommandResult {
    let channel = msg.channel_id;
    let mut data = ctx.data.write().await;
    let games = data.get_mut::<BgGames>().unwrap();
    if games.contains_key(&channel) {
        games.get_mut(&channel).unwrap().restart()?;
    } else {
        let game = BackGroundGame::new().await;
        game.start(ctx, channel, mapsets).await;
        games.insert(channel, game);
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
    let response = if let Some(hint) = hint {
        msg.channel_id.say(ctx, hint).await?
    } else {
        msg.channel_id
            .say(
                ctx,
                "There is no running game in this channel, \
                start one with `<bg s`",
            )
            .await?
    };
    response.reaction_delete(ctx, msg.author.id).await;
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
            .send_message(ctx, |m| {
                let bytes: &[u8] = &img;
                m.add_file((bytes, "bg_img.png"))
            })
            .await?;
    } else {
        msg.channel_id
            .say(
                ctx,
                "There is no running game in this channel, \
                start one with `<bg s`",
            )
            .await?;
    }
    Ok(())
}

#[command]
#[aliases("end")]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    let channel = msg.channel_id;
    let mut data = ctx.data.write().await;
    let games = data.get_mut::<BgGames>().unwrap();
    if !games.contains_key(&channel) {
        msg.channel_id
            .say(
                ctx,
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
    response.reaction_delete(ctx, msg.author.id).await;
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
        mysql.all_bggame_scores()?
    };
    if !global && msg.guild_id.is_some() {
        let guild_id = msg.guild_id.unwrap();
        let member_ids = ctx
            .cache
            .guild_field(guild_id, |guild| {
                guild.members.keys().map(|id| id.0).collect::<Vec<_>>()
            })
            .await;
        if let Some(members) = member_ids {
            scores.retain(|(user, _)| members.iter().any(|member| member == user));
        }

        if scores.is_empty() {
            msg.channel_id
                .say(
                    ctx,
                    "Looks like no one on this server has played the backgroundgame yet",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
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
    let data = BGRankingEmbed::new(author_idx, initial_scores, global, 1, (1, pages));

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Skip pagination if too few entries
    if scores.len() <= 15 {
        resp.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination =
        BGRankingPagination::new(ctx, resp, msg.author.id, author_idx, scores, global).await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

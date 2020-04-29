#![allow(unused_imports)]

mod game;
mod hints;
mod img_reveal;
mod util;

pub use game::BackGroundGame;
use hints::Hints;
use img_reveal::ImageReveal;

use crate::{util::discord, BgGames, Error, MySQL};

use rosu::models::GameMode;
use serenity::{
    collector::MessageCollectorBuilder,
    framework::standard::{macros::command, Args, CommandResult},
    http::client::Http,
    model::{id::ChannelId, prelude::Message},
    prelude::{Context, RwLock as SRwLock, ShareMap},
};
use std::{collections::VecDeque, fmt::Write, sync::Arc, time::Duration};

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
Subcommands can be abbreviated with `s, h, b, r`, e.g. `<bg h` for hints.\n\
With `<bg start mania` (`<bg s m`) I will give mania backgrounds to guess."]
#[aliases("bg")]
#[sub_commands("start", "hint", "bigger", "stats")]
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
            or `<bg stats` to check your correct guesses.\n\
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
            format!("Looks like you haven't guessed any backgrounds yet"),
        )
        .await?
    };
    discord::reaction_deletion(&ctx, response, msg.author.id);
    Ok(())
}

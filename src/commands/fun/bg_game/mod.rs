mod game;
mod hints;
mod img_reveal;
mod util;

pub use game::BackGroundGame;
use hints::Hints;
use img_reveal::ImageReveal;

use crate::{util::discord, BgGameKey, DispatchEvent, DispatcherKey, Error, MySQL};

use hey_listen::RwLock;
use rosu::models::GameMode;
use serenity::{
    framework::standard::{macros::command, CommandResult},
    http::client::Http,
    model::{id::ChannelId, prelude::Message},
    prelude::{Context, RwLock as SRwLock, ShareMap},
};
use std::sync::Arc;

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
#[sub_commands("start", "hint", "bigger", "stop", "stats")]
async fn backgroundgame(ctx: &mut Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(
            &ctx.http,
            "Use `<bg s` to (re)start the game, \
        `<bg b` to increase the image, \
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
#[aliases("s", "resolve", "r")]
#[sub_commands("mania")]
async fn start(ctx: &mut Context, msg: &Message) -> CommandResult {
    _start(GameMode::STD, ctx, msg).await
}

#[command]
#[aliases("m")]
async fn mania(ctx: &mut Context, msg: &Message) -> CommandResult {
    _start(GameMode::MNA, ctx, msg).await
}

async fn _start(mode: GameMode, ctx: &mut Context, msg: &Message) -> CommandResult {
    let game_exists = {
        let data = ctx.data.read().await;
        data.get::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .contains_key(&msg.channel_id)
    };
    if !game_exists {
        let game = BackGroundGame::new(ctx, msg.channel_id, mode == GameMode::STD);
        let game = Arc::new(RwLock::new(game));
        let mut data = ctx.data.write().await;
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .insert(msg.channel_id, Arc::clone(&game));
        let dispatcher = data
            .get_mut::<DispatcherKey>()
            .expect("Could not get DispatcherKey")
            .clone();
        dispatcher.write().await.add_listener(
            DispatchEvent::BgMsgEvent {
                channel: msg.channel_id,
                user: msg.author.id,    // irrelevant
                content: String::new(), // irrelevant
            },
            &game,
        );
    }
    let game = {
        let mut data = ctx.data.write().await;
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGamekey")
            .get(&msg.channel_id)
            .unwrap()
            .clone()
    };
    let restart_future = { game.write().restart() };
    restart_future.await?;
    Ok(())
}

#[command]
async fn stop(ctx: &mut Context, msg: &Message) -> CommandResult {
    _stop(
        &Arc::clone(&ctx.data),
        &Arc::clone(&ctx.http),
        msg.channel_id,
    )
    .await
}

async fn _stop(
    data: &Arc<SRwLock<ShareMap>>,
    http: &Arc<Http>,
    channel: ChannelId,
) -> CommandResult {
    let removing = {
        let data = data.read().await;
        let game = data
            .get::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .get(&channel);
        if let Some(game) = game {
            let resolve_future = { game.read().resolve(None) };
            resolve_future.await?;
        }
        game.is_some()
    };
    if removing {
        let mut data = data.write().await;
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .remove(&channel);
    };
    if removing {
        channel
            .say(&http, "End of game, see you next time o/")
            .await?;
    } else {
        channel
            .say(&http, "There was no running game in this channel")
            .await?;
    }
    Ok(())
}

#[command]
#[aliases("h")]
async fn hint(ctx: &mut Context, msg: &Message) -> CommandResult {
    let hint = {
        let mut data = ctx.data.write().await;
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .get(&msg.channel_id)
            .map(|game| game.write().hint())
    };
    let _ = if let Some(hint) = hint {
        msg.channel_id.say(&ctx.http, hint).await?
    } else {
        msg.channel_id
            .say(&ctx.http, "There is no running game in this channel")
            .await?
    };
    Ok(())
}

#[command]
#[aliases("b", "enhance")]
async fn bigger(ctx: &mut Context, msg: &Message) -> CommandResult {
    let img: Option<Result<Vec<u8>, Error>> = {
        let mut data = ctx.data.write().await;
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .get_mut(&msg.channel_id)
            .map(|game| game.write().increase_sub_image())
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
            .say(&ctx.http, "There is no running game in this channel")
            .await?;
    }
    Ok(())
}

#[command]
async fn stats(ctx: &mut Context, msg: &Message) -> CommandResult {
    let score = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.get_bggame_score(msg.author.id.0)?
    };
    let response = msg
        .reply(
            (&ctx.cache, &*ctx.http),
            format!("You've guessed {} backgrounds correctly!", score),
        )
        .await?;
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone()).await;
    Ok(())
}

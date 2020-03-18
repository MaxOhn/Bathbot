mod game;
mod hints;
mod img_reveal;
mod util;

pub use game::BackGroundGame;
use hints::Hints;
use img_reveal::ImageReveal;

use crate::{util::discord, BgGameKey, DispatchEvent, DispatcherKey, Error};

use hey_listen::RwLock;
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::sync::Arc;

#[command]
#[description = "Play the background game!\
With `<bg start` you can start the game in a channel \
which makes me post a piece of some map's background. \
Then you have to guess the **title** of the map's song.\n\
With `<bg hint` I will give you some tips (repeatable).\n\
With `<bg bigger` I will increase the size of the revealed part.\n\
With `<bg resolve` I will show you the solution.\n\
With `<bg stop` I will stop the game in this channel."]
#[aliases("bg")]
#[sub_commands("start", "hint", "bigger", "stop")]
fn backgroundgame(ctx: &mut Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(
        &ctx.http,
        "Use `<bg s` to (re)start the game, \
        `<bg b` to increase the image, \
        `<bg h` to get a hint, \
        or `<bg stop` to stop the game",
    )?;
    Ok(())
}

#[command]
#[aliases("s", "resolve", "r")]
fn start(ctx: &mut Context, msg: &Message) -> CommandResult {
    let game_exists = {
        let data = ctx.data.read();
        data.get::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .contains_key(&msg.channel_id)
    };
    if !game_exists {
        let game = BackGroundGame::new(ctx, msg.channel_id);
        let game = Arc::new(RwLock::new(game));
        let mut data = ctx.data.write();
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .insert(msg.channel_id, Arc::clone(&game));
        let dispatcher = data
            .get_mut::<DispatcherKey>()
            .expect("Could not get DispatcherKey")
            .clone();
        dispatcher.write().add_listener(
            DispatchEvent::BgMsgEvent {
                channel: msg.channel_id,
                user: msg.author.id,    // irrelevant
                content: String::new(), // irrelevant
            },
            &game,
        );
    }
    let game = {
        let mut data = ctx.data.write();
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGamekey")
            .get(&msg.channel_id)
            .unwrap()
            .clone()
    };
    game.write().restart()?;
    Ok(())
}

#[command]
fn stop(ctx: &mut Context, msg: &Message) -> CommandResult {
    let removing = {
        let mut data = ctx.data.write();
        let game = data
            .get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .get_mut(&msg.channel_id);
        if let Some(game) = game {
            game.write().resolve(None)?;
            true
        } else {
            false
        }
    };
    if removing {
        let mut data = ctx.data.write();
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .remove(&msg.channel_id);
    };
    if removing {
        msg.channel_id
            .say(&ctx.http, "End of game, see you next time o/")?;
    } else {
        msg.channel_id
            .say(&ctx.http, "There was no running game in this channel")?;
    }
    Ok(())
}

#[command]
#[aliases("h")]
fn hint(ctx: &mut Context, msg: &Message) -> CommandResult {
    let hint = {
        let mut data = ctx.data.write();
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .get(&msg.channel_id)
            .map(|game| game.write().hint())
    };
    let response = if let Some(hint) = hint {
        msg.channel_id.say(&ctx.http, hint)?
    } else {
        msg.channel_id
            .say(&ctx.http, "There is no running game in this channel")?
    };
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
#[aliases("b", "enhance")]
fn bigger(ctx: &mut Context, msg: &Message) -> CommandResult {
    let img: Option<Result<Vec<u8>, Error>> = {
        let mut data = ctx.data.write();
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .get_mut(&msg.channel_id)
            .map(|game| game.write().increase_sub_image())
    };
    if let Some(Ok(img)) = img {
        msg.channel_id.send_message(&ctx.http, |m| {
            let bytes: &[u8] = &img;
            m.add_file((bytes, "bg_img.png"))
        })?;
    } else {
        msg.channel_id
            .say(&ctx.http, "There is no running game in this channel")?;
    }
    Ok(())
}

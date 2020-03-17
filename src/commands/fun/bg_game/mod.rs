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
    framework::standard::{macros::command, Args, CommandResult},
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
#[sub_commands("start", "hint", "bigger", "resolve", "stop")]
fn backgroundgame(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    start(ctx, msg, args)
}

#[command]
#[aliases("s")]
fn start(ctx: &mut Context, msg: &Message) -> CommandResult {
    let game_running = {
        let data = ctx.data.read();
        data.get::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .contains_key(&msg.channel_id)
    };
    if game_running {
        resolve_only(ctx, msg)?;
    }
    let game = BackGroundGame::new(ctx, msg.channel_id, Arc::clone(&ctx.http))?;
    let game = Arc::new(RwLock::new(game));
    let dispatcher = {
        let mut data = ctx.data.write();
        let games = data
            .get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey");
        games.insert(msg.channel_id, Arc::clone(&game));
        data.get_mut::<DispatcherKey>()
            .expect("Could not get DispatcherKey")
            .clone()
    };
    dispatcher.write().add_listener(
        DispatchEvent::BgMsgEvent {
            channel: msg.channel_id,
            user: msg.author.id,    // irrelevant
            content: String::new(), // irrelevant
        },
        &game,
    );
    let img = game.read().sub_image()?;
    let response = msg.channel_id.send_message(&ctx.http, |m| {
        let bytes: &[u8] = &img;
        m.content("Here's the next one:")
            .add_file((bytes, "bg_img.png"))
    })?;
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
fn stop(ctx: &mut Context, msg: &Message) -> CommandResult {
    let removing = {
        let present = {
            let data = ctx.data.read();
            data.get::<BgGameKey>()
                .expect("Could not get BgGameKey")
                .contains_key(&msg.channel_id)
        };
        if present {
            resolve_only(ctx, msg)?;
        }
        present
    };
    if removing {
        let mut data = ctx.data.write();
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .remove(&msg.channel_id);
    };
    let response = if removing {
        msg.channel_id
            .say(&ctx.http, "End of game, see you next time o/")?
    } else {
        msg.channel_id
            .say(&ctx.http, "There was no running game in this channel")?
    };
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
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
    let increased_radius = {
        let mut data = ctx.data.write();
        data.get_mut::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .get(&msg.channel_id)
            .map(|game| {
                game.write().increase_radius();
                true
            })
            .is_some()
    };
    let response = if increased_radius {
        let img: Option<Result<Vec<u8>, Error>> = {
            let data = ctx.data.read();
            data.get::<BgGameKey>()
                .expect("Could not get BgGameKey")
                .get(&msg.channel_id)
                .map(|game| game.read().sub_image())
        };
        if let Some(Ok(img)) = img {
            Some(msg.channel_id.send_message(&ctx.http, |m| {
                let bytes: &[u8] = &img;
                m.add_file((bytes, "bg_img.png"))
            })?)
        } else {
            None
        }
    } else {
        Some(
            msg.channel_id
                .say(&ctx.http, "There is no running game in this channel")?,
        )
    };
    if let Some(response) = response {
        discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    }
    Ok(())
}

#[command]
#[aliases("solve", "r")]
fn resolve(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    resolve_only(ctx, msg)?;
    start(ctx, msg, args)
}

fn resolve_only(ctx: &mut Context, msg: &Message) -> CommandResult {
    let game_running = {
        let data = ctx.data.read();
        data.get::<BgGameKey>()
            .expect("Could not get BgGameKey")
            .contains_key(&msg.channel_id)
    };
    let response = if game_running {
        let result: Option<(Vec<u8>, u32)> = {
            let data = ctx.data.read();
            if let Some(game) = data
                .get::<BgGameKey>()
                .expect("Could not get BgGameKey")
                .get(&msg.channel_id)
            {
                let game = game.read();
                Some((game.reveal()?, game.mapset_id))
            } else {
                None
            }
        };
        {
            let mut data = ctx.data.write();
            data.get_mut::<BgGameKey>()
                .expect("Could not get BgGameKey")
                .remove(&msg.channel_id);
        }
        if let Some((img, mapset_id)) = result {
            Some(msg.channel_id.send_message(&ctx.http, |m| {
                let bytes: &[u8] = &img;
                m.add_file((bytes, "bg_img.png")).content(format!(
                    "Full background: https://osu.ppy.sh/beatmapsets/{}",
                    mapset_id
                ))
            })?)
        } else {
            None
        }
    } else {
        Some(
            msg.channel_id
                .say(&ctx.http, "There is no running game in this channel")?,
        )
    };
    if let Some(response) = response {
        discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    }
    Ok(())
}

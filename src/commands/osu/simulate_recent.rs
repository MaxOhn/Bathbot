use crate::{
    commands::osu::MINIMIZE_DELAY,
    messages::{BotEmbed, SimulateData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{OsuArgs, OsuRequest, UserRecentArgs},
    models::{GameMode, Score},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::thread;
use tokio::runtime::Runtime;

fn simulate_recent_send(
    mode: GameMode,
    ctx: &mut Context,
    msg: &Message,
    mut args: Args,
) -> CommandResult {
    let name: String = if args.is_empty() {
        let data = ctx.data.read();
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id.say(
                    &ctx.http,
                    "Either specify an osu name or link your discord \
                     to an osu profile via `<link osuname`",
                )?;
                return Ok(());
            }
        }
    } else {
        args.single_quoted()?
    };
    let recent_args = UserRecentArgs::with_username(&name).mode(mode).limit(1);
    let recent_req: OsuRequest<Score> = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        osu.create_request(OsuArgs::Recent(recent_args))
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the recent score
    let score: Score = match rt.block_on(recent_req.queue()) {
        Ok(mut scores) => {
            if let Some(score) = scores.pop() {
                score
            } else {
                msg.channel_id.say(
                    &ctx.http,
                    format!("No recent plays found for user `{}`", name),
                )?;
                return Ok(());
            }
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(CommandError(format!(
                "Error while retrieving UserRecent: {}",
                why
            )));
        }
    };

    // Retrieving the score's beatmap
    let res = rt.block_on(async {
        score
            .beatmap
            .as_ref()
            .unwrap()
            .get(mode)
            .await
            .map_err(|e| {
                CommandError(format!(
                    "Error while retrieving LazilyLoaded<Beatmap> of recent: {}",
                    e
                ))
            })
    });
    let map = match res {
        Ok(map) => map,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let data = SimulateData::new(Some(score), map, mode, ctx.cache.clone());

    // Creating the embed
    let embed = BotEmbed::SimulateScore(&data);
    let mut msg = msg.channel_id.send_message(&ctx.http, |m| {
        m.content("Simulated score:").embed(|e| embed.create(e))
    })?;
    let embed = BotEmbed::SimulateScoreMini(Box::new(data));
    msg.edit(&ctx, |m| {
        thread::sleep(MINIMIZE_DELAY);
        m.embed(|e| embed.create(e))
    })?;
    Ok(())
}

#[command]
#[description = "Display an unchoked version of user's most recent play"]
#[usage = "badewanne3"]
#[aliases("sr")]
pub fn simulaterecent(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    simulate_recent_send(GameMode::STD, ctx, msg, args)
}

/*
// TODO
#[command]
#[description = "Display an unchoked version of user's most recent mania play"]
#[usage = "badewanne3"]
#[aliases("srm")]
pub fn simulaterecentmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    simulate_recent_send(GameMode::MNA, ctx, msg, args)
}
*/

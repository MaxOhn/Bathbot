use crate::{
    commands::arguments,
    messages::{BotEmbed, ScoreMultiData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{BeatmapArgs, OsuArgs, OsuRequest, ScoreArgs, UserArgs},
    models::{Beatmap, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::error::Error;
use tokio::runtime::Runtime;

#[command]
#[description = "Display scores for all mods that a user has on a map. \
                 Beatmap can be given as url or just **mapid**. \
                 If no beatmap is given, it will choose the map of a score in the channel's history"]
#[example = "badewanne3 2240404"]
#[aliases("c", "compare")]
fn scores(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the name
    let name: String = match args.len() {
        0 => {
            msg.channel_id.say(
                &ctx.http,
                "You need to provide a decimal number as argument",
            )?;
            return Ok(());
        }
        1 => {
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
        }
        _ => args.single_quoted()?,
    };

    // Parse the beatmap id
    let map_id = if let Some(map_id) = arguments::get_beatmap_id(args.single::<String>()?) {
        map_id
    } else {
        msg.channel_id.say(
            &ctx.http,
            "If no osu name is provided, the first argument must be a beatmap id. \
             If you want to give an osu name, do so as first argument. \
             The second argument should then be the beatmap id. \
             The beatmap id can be given as number or as URL to the beatmap.",
        )?;
        return Ok(());
    };
    let score_args = ScoreArgs::with_map_id(map_id).username(&name);
    let user_args = UserArgs::with_username(&name);
    let map_args = BeatmapArgs::new().map_id(map_id);
    let (score_req, user_req, map_req): (OsuRequest<Score>, OsuRequest<User>, OsuRequest<Beatmap>) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let score_req = osu.create_request(OsuArgs::Scores(score_args));
        let user_req = osu.create_request(OsuArgs::Users(user_args));
        let map_req = osu.create_request(OsuArgs::Beatmaps(map_args));
        (score_req, user_req, map_req)
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve map, user, and user's scores on the map
    let res = rt.block_on(async {
        let users = user_req
            .queue()
            .await
            .or_else(|e| Err(CommandError(format!("Error while retrieving Users: {}", e))))?;
        let scores = score_req.queue().await.or_else(|e| {
            Err(CommandError(format!(
                "Error while retrieving Scores: {}",
                e
            )))
        })?;
        let maps = map_req.queue().await.or_else(|e| {
            Err(CommandError(format!(
                "Error while retrieving Beatmaps: {}",
                e
            )))
        })?;
        Ok((scores, users, maps))
    });
    let (scores, user, map) = match res {
        Ok((scores, mut users, mut maps)) => {
            let user = match users.pop() {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))?;
                    return Ok(());
                }
            };
            let map = match maps.pop() {
                Some(map) => map,
                None => {
                    msg.channel_id.say(
                        &ctx.http,
                        format!("Beatmap with id {} was not found", map_id),
                    )?;
                    return Ok(());
                }
            };
            (scores, user, map)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let data = match ScoreMultiData::new(map.mode, user, map, scores, ctx.cache.clone()) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating scores data, blame bade",
            )?;
            return Err(CommandError::from(why.description()));
        }
    };

    // Creating the embed
    let embed = BotEmbed::UserScoreMulti(Box::new(data));
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| embed.create(e)));
    Ok(())
}

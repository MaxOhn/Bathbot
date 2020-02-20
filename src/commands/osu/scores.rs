use crate::{
    commands::arguments,
    messages::{BotEmbed, ScoreMultiData},
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::backend::requests::{BeatmapRequest, ScoreRequest, UserRequest};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
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
            "If no osu name is provided, the first argument must be a beatmap id.\n\
             If you want to give an osu name, do so as first argument.\n\
             The second argument should then be the beatmap id.\n\
             The beatmap id can be given as number or as URL to the beatmap.",
        )?;
        return Ok(());
    };

    // Retrieve user, map, and user's scores on the map
    let (user, map, scores) = {
        let mut rt = Runtime::new().unwrap();
        let map_req = BeatmapRequest::new().map_id(map_id);
        let score_req = ScoreRequest::with_map_id(map_id).username(&name);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let map = match rt.block_on(map_req.queue_single(osu)) {
            Ok(result) => match result {
                Some(map) => map,
                None => {
                    msg.channel_id.say(
                        &ctx.http,
                        format!("Could not find beatmap with id `{}`", map_id),
                    )?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let scores = match rt.block_on(score_req.queue(osu)) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        let user_req = UserRequest::with_username(&name).mode(map.mode);
        let user = match rt.block_on(user_req.queue_single(osu)) {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("Could not find user `{}`", name))?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        };
        (user, map, scores)
    };

    // Accumulate all necessary data
    let data = match ScoreMultiData::new(map.mode, user, map, scores, ctx.cache.clone()) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating scores data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Creating the embed
    let embed = BotEmbed::UserScoreMulti(Box::new(data));
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| embed.create(e)));
    Ok(())
}

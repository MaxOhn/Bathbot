use crate::{
    arguments::NameMapArgs,
    database::MySQL,
    embeds::BasicEmbedData,
    util::{discord, globals::OSU_API_ISSUE},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::{BeatmapRequest, ScoreRequest, UserRequest},
    models::ApprovalStatus::{Loved, Ranked},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;

#[command]
#[description = "Display a user's top score for each mod on a given map. \
                 If no map is given, I will choose the last map \
                 I can find in my embeds of this channel"]
#[usage = "[username] [map url / map id]"]
#[example = "badewanne3"]
#[example = "badewanne3 2240404"]
#[example = "badewanne3 https://osu.ppy.sh/beatmapsets/902425#osu/2240404"]
#[aliases("c", "compare")]
fn scores(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = NameMapArgs::new(args);
    let map_id = if let Some(map_id) = args.map_id {
        map_id
    } else {
        let msgs = msg
            .channel_id
            .messages(&ctx.http, |retriever| retriever.limit(50))?;
        match discord::map_id_from_history(msgs, ctx.cache.clone()) {
            Some(id) => id,
            None => {
                msg.channel_id.say(
                    &ctx.http,
                    "No map embed found in this channel's recent history.\n\
                     Try specifying a map as last argument either by url to the map, \
                     or just by map id.",
                )?;
                return Ok(());
            }
        }
    };
    let name = if let Some(name) = args.name {
        name
    } else {
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
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieving the beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        match mysql.get_beatmap(map_id) {
            Ok(map) => (false, map),
            Err(_) => {
                let map_req = BeatmapRequest::new().map_id(map_id);
                let osu = data.get::<Osu>().expect("Could not get osu client");
                let map = match rt.block_on(map_req.queue_single(&osu)) {
                    Ok(result) => match result {
                        Some(map) => map,
                        None => {
                            msg.channel_id.say(
                                &ctx.http,
                                format!("Could not find beatmap with id `{}`. Did you give me a mapset id instead of a map id?", map_id),
                            )?;
                            return Ok(());
                        }
                    },
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                        return Err(CommandError::from(why.to_string()));
                    }
                };
                (
                    map.approval_status == Ranked || map.approval_status == Loved,
                    map,
                )
            }
        }
    };

    // Retrieve user and user's scores on the map
    let (user, map, scores) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let score_req = ScoreRequest::with_map_id(map_id)
            .username(&name)
            .mode(map.mode);
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
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let data = match BasicEmbedData::create_scores(user, map, scores, &ctx) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating scores data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Sending the embed
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)));

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of compare command to database: {}", why);
        }
    }

    // Save the response owner
    discord::save_response_owner(response?.id, msg.author.id, ctx.data.clone());
    Ok(())
}

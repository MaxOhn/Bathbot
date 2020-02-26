use crate::{
    commands::arguments,
    commands::{ArgParser, ModSelection},
    database::MySQL,
    messages::BasicEmbedData,
    scraper::Scraper,
    util::{globals::OSU_API_ISSUE, osu},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::BeatmapRequest,
    models::{
        ApprovalStatus::{Loved, Ranked},
        GameMods,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;

fn leaderboard_send(
    national: bool,
    ctx: &mut Context,
    msg: &Message,
    mut args: Args,
) -> CommandResult {
    let author_name = {
        let data = ctx.data.read();
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        links.get(msg.author.id.as_u64()).cloned()
    };
    // Parse the beatmap id
    let map_id = if args.is_empty() {
        let msgs = msg
            .channel_id
            .messages(&ctx.http, |retriever| retriever.limit(50))?;
        match osu::map_id_from_history(msgs, ctx.cache.clone()) {
            Some(id) => id,
            None => {
                msg.channel_id.say(
                    &ctx.http,
                    "No map embed found in this channel's recent history.\n\
                     Try specifying a map either by url to the map, or just by map id.",
                )?;
                return Ok(());
            }
        }
    } else {
        let first_str = args.single::<String>()?;
        if let Some(id) = arguments::get_regex_id(&first_str) {
            id
        } else {
            let msgs = msg
                .channel_id
                .messages(&ctx.http, |retriever| retriever.limit(50))?;
            match osu::map_id_from_history(msgs, ctx.cache.clone()) {
                Some(id) => id,
                None => {
                    msg.channel_id.say(
                        &ctx.http,
                        "No beatmap specified and none found in recent channel history. \
                         Try specifying a map either by url to the map, or just by map id.",
                    )?;
                    return Ok(());
                }
            }
        }
    };
    let mut arg_parser = ArgParser::new(args);
    let (mods, selection) = arg_parser
        .get_mods()
        .unwrap_or_else(|| (GameMods::default(), ModSelection::None));
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

    // Retrieve the map's leaderboard
    let scores = {
        let data = ctx.data.read();
        let scraper = data.get::<Scraper>().expect("Could not get Scraper");
        let scores_future = scraper.get_leaderboard(
            map_id,
            national,
            match selection {
                ModSelection::Excludes | ModSelection::None => None,
                _ => Some(&mods),
            },
        );
        match rt.block_on(scores_future) {
            Ok(scores) => scores,
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let amount = scores.len();
    let scores: Vec<_> = scores.into_iter().take(10).collect();

    // Accumulate all necessary data
    let map_copy = if map_to_db { Some(map.clone()) } else { None };
    let _ = msg.channel_id.broadcast_typing(&ctx.http);
    let data = match BasicEmbedData::create_leaderboard(author_name, map, scores, &ctx) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating leaderboard data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Sending the embed
    msg.channel_id.send_message(&ctx.http, |m| {
        let mut content = format!(
            "I found {} scores with the specified mods on the map's leaderboard",
            amount
        );
        if amount > 10 {
            content.push_str(", here's the top 10 of them:");
        } else {
            content.push(':');
        }
        m.content(content).embed(|e| data.build(e))
    })?;

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of recent command to database: {}", why);
        }
    }
    Ok(())
}

#[command]
#[description = "Display the national leaderboard of a map"]
#[example = "2240404"]
#[aliases("lb")]
pub fn leaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    leaderboard_send(true, ctx, msg, args)
}

#[command]
#[description = "Display the global leaderboard of a map"]
#[example = "2240404"]
#[aliases("glb")]
pub fn globalleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    leaderboard_send(false, ctx, msg, args)
}

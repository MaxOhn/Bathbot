use crate::{
    commands::{ArgParser, ModSelection},
    database::MySQL,
    messages::{BotEmbed, LeaderboardData},
    scraper::Scraper,
    util::globals::OSU_API_ISSUE,
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::RecentRequest,
    models::{
        ApprovalStatus::{Loved, Ranked},
        GameMode, GameMods,
    },
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::convert::TryFrom;
use tokio::runtime::Runtime;

fn recent_lb_send(mode: GameMode, ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let mut arg_parser = ArgParser::new(args);
    let (mods, selection) = if let Some((m, s)) = arg_parser.get_mods() {
        let mods = match GameMods::try_from(m.as_ref()) {
            Ok(mods) => mods,
            Err(_) => {
                msg.channel_id
                    .say(&ctx.http, "Could not parse given mods")?;
                return Ok(());
            }
        };
        (mods, s)
    } else {
        (GameMods::default(), ModSelection::None)
    };
    let national = !arg_parser.get_global();
    let author_name = {
        let data = ctx.data.read();
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        links.get(msg.author.id.as_u64()).cloned()
    };
    let name: String = if let Some(name) = arg_parser.get_name() {
        name
    } else {
        match author_name.as_ref() {
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

    // Retrieve the recent scores
    let score = {
        let request = RecentRequest::with_username(&name).mode(mode).limit(1);
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        match rt.block_on(request.queue(osu)) {
            Ok(mut score) => {
                if let Some(score) = score.pop() {
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
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let map_id = score.beatmap_id.unwrap();

    // Retrieving the score's beatmap
    let (map_to_db, map) = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        match mysql.get_beatmap(map_id) {
            Ok(map) => (false, map),
            Err(_) => {
                let osu = data.get::<Osu>().expect("Could not get osu client");
                let map = match rt.block_on(score.get_beatmap(osu)) {
                    Ok(m) => m,
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
            // TODO: fix mods
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
    let data = match LeaderboardData::new(author_name, map, scores, &ctx) {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while calculating leaderboard data, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    };

    // Creating the embed
    let embed = BotEmbed::Leaderboard(data);
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
        m.content(content).embed(|e| embed.create(e))
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
#[description = "Display the leaderboard of a map that a user recently played"]
#[usage = "badewanne3"]
#[aliases("rlb")]
pub fn recentleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display the leaderboard of a map that a mania user recently played"]
#[usage = "badewanne3"]
#[aliases("rmlb")]
pub fn recentmanialeaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display the leaderboard of a map that a taiko user recently played"]
#[usage = "badewanne3"]
#[aliases("rtlb")]
pub fn recenttaikoleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display the leaderboard of a map that a ctb user recently played"]
#[usage = "badewanne3"]
#[aliases("rclb")]
pub fn recentctbleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::CTB, ctx, msg, args)
}

use crate::{
    arguments::{ModSelection, NameModArgs},
    database::MySQL,
    embeds::BasicEmbedData,
    scraper::Scraper,
    util::{discord, globals::OSU_API_ISSUE},
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
use tokio::runtime::Runtime;

fn recent_lb_send(
    mode: GameMode,
    national: bool,
    ctx: &mut Context,
    msg: &Message,
    args: Args,
) -> CommandResult {
    let author_name = {
        let data = ctx.data.read();
        let links = data
            .get::<DiscordLinks>()
            .expect("Could not get DiscordLinks");
        links.get(msg.author.id.as_u64()).cloned()
    };
    let args = NameModArgs::new(args);
    let (mods, selection) = args
        .mods
        .unwrap_or_else(|| (GameMods::default(), ModSelection::None));
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
    let response = msg.channel_id.send_message(&ctx.http, |m| {
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
    });

    // Add map to database if its not in already
    if let Some(map) = map_copy {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.insert_beatmap(&map) {
            warn!("Could not add map of recent command to database: {}", why);
        }
    }

    // Save the response owner
    discord::save_response_owner(response?.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
#[description = "Display the belgian leaderboard of a map \
                 that a user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rlb")]
pub fn recentleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::STD, true, ctx, msg, args)
}

#[command]
#[description = "Display the belgian leaderboard of a map \
                 that a mania user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rmlb")]
pub fn recentmanialeaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::MNA, true, ctx, msg, args)
}

#[command]
#[description = "Display the belgian leaderboard of a map \
                 that a taiko user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rtlb")]
pub fn recenttaikoleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::TKO, true, ctx, msg, args)
}

#[command]
#[description = "Display the belgian leaderboard of a map \
                 that a ctb user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rclb")]
pub fn recentctbleaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::CTB, true, ctx, msg, args)
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rglb")]
pub fn recentgloballeaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::STD, false, ctx, msg, args)
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a mania user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rmglb")]
pub fn recentmaniagloballeaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::MNA, false, ctx, msg, args)
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a taiko user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rtglb")]
pub fn recenttaikogloballeaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::TKO, false, ctx, msg, args)
}

#[command]
#[description = "Display the global leaderboard of a map \
                 that a ctb user recently played. Mods can be specified"]
#[usage = "[username] [+mods]"]
#[example = "badewanne3 +hdhr"]
#[aliases("rcglb")]
pub fn recentctbgloballeaderboard(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_lb_send(GameMode::CTB, false, ctx, msg, args)
}

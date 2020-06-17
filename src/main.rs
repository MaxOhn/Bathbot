mod arguments;
mod commands;
pub mod database;
mod embeds;
mod events;
pub mod pagination;
pub mod roppai;
mod scraper;
mod streams;
pub mod structs;
pub mod util;

use crate::scraper::Scraper;
use commands::{fun::*, help::*, osu::*, owner::*, streams::*, utility::*};
use database::MySQL;
use events::Handler;
use streams::Twitch;
use structs::Osu;
use structs::*;
pub use util::{discord::get_member, Error, MessageExt};

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use chrono::{Local, Utc};
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;
use rosu::backend::Osu as OsuClient;
use serenity::{
    framework::{
        standard::{macros::hook, CommandResult, DispatchError},
        StandardFramework,
    },
    http::Http,
    model::channel::{Channel, Message},
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Arc,
};

// Will create an async worker to regularly check for online twitch streams
pub const WITH_STREAM_TRACK: bool = false;
// Will make the scraper use the osu_session cookie of an osu! account
pub const WITH_SCRAPER: bool = false;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Could not load .env file");
    let colors = ColoredLevelConfig::new()
        .info(Color::Green)
        .debug(Color::Blue)
        .warn(Color::Yellow)
        .error(Color::Red);
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                Local::now().format("[%m-%d %H:%M:%S]"),
                colors.color(record.level()),
                message
            ))
        })
        .level(LevelFilter::Info)
        .level_for("bathbot", LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(
            fern::log_file(&format!(
                "logs/log-{}.log",
                Utc::now().format("%F-%H-%M-%S").to_string()
            ))
            .expect("Could prepare log file"),
        )
        .apply()
        .expect("Could not prepare fern-logger");

    // -----------------
    // Data preparations
    // -----------------

    // Discord
    let discord_token = env::var("DISCORD_TOKEN").expect("Could not load DISCORD_TOKEN");
    let http = Http::new_with_token(&discord_token);

    // Database
    let database_url = env::var("DATABASE_URL").expect("Could not load DATABASE_URL");
    let mysql =
        MySQL::new(&database_url).unwrap_or_else(|why| panic!("Could not create MySQL: {}", why));

    // Osu
    let osu_token = env::var("OSU_TOKEN").expect("Could not load OSU_TOKEN");
    let osu = OsuClient::new(osu_token);
    let discord_links = mysql
        .get_discord_links()
        .unwrap_or_else(|why| panic!("Could not get discord_links: {}", why));

    // Scraper
    let scraper = Scraper::new()
        .await
        .unwrap_or_else(|why| panic!("Could not create Scraper: {}", why));

    // Stream tracking
    let twitch_users = mysql
        .get_twitch_users()
        .unwrap_or_else(|why| panic!("Could not get twitch_users: {}", why));
    let stream_tracks = mysql
        .get_stream_tracks()
        .unwrap_or_else(|why| panic!("Could not get stream_tracks: {}", why));
    let twitch_client_id = env::var("TWITCH_CLIENT_ID").expect("Could not load TWITCH_CLIENT_ID");
    let twitch_token = env::var("TWITCH_TOKEN").expect("Could not load TWITCH_TOKEN");
    let twitch = if WITH_STREAM_TRACK {
        Some(
            Twitch::new(&twitch_client_id, &twitch_token)
                .await
                .unwrap_or_else(|why| panic!("Could not create Twitch: {}", why)),
        )
    } else {
        None
    };

    // Individual guild settings
    let guilds = mysql
        .get_guilds()
        .unwrap_or_else(|why| panic!("Could not get Guilds: {}", why));

    // General
    let owners = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);
            owners
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };
    let now = Utc::now();

    // Custom (temporal(?)) manual user verification
    let verified_users = mysql
        .get_bg_verified()
        .expect("Could not get verified users");

    // ---------------
    // Framework setup
    // ---------------

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefixes(vec!["<", "!!"])
                .delimiter(' ')
                .case_insensitivity(true)
                .ignore_bots(true)
                .no_dm_prefix(true)
                .owners(owners)
        })
        .before(before)
        .after(after)
        .on_dispatch_error(dispatch_error)
        .bucket("songs", |b| b.delay(20).limit(1))
        .await
        .bucket("bg_start", |b| b.time_span(30).limit(4))
        .await
        .bucket("bg_bigger", |b| b.time_span(10).limit(3))
        .await
        .bucket("bg_hint", |b| b.time_span(7).limit(3))
        .await
        .help(&HELP)
        .group(&OSUGENERAL_GROUP)
        .group(&OSU_GROUP)
        .group(&MANIA_GROUP)
        .group(&TAIKO_GROUP)
        .group(&CATCHTHEBEAT_GROUP)
        .group(&FUN_GROUP)
        .group(&UTILITY_GROUP)
        .group(&STREAMTRACKING_GROUP)
        .group(&OWNER_GROUP);

    let mut discord = Client::new(&discord_token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Could not create discord client");

    // Insert everything
    {
        let mut data = discord.data.write().await;
        data.insert::<CommandCounter>(HashMap::default());
        data.insert::<Osu>(osu);
        data.insert::<Scraper>(scraper);
        data.insert::<MySQL>(mysql);
        data.insert::<DiscordLinks>(discord_links);
        data.insert::<BootTime>(now);
        data.insert::<PerformanceCalculatorLock>(Arc::new(Mutex::new(())));
        data.insert::<TwitchUsers>(twitch_users);
        data.insert::<StreamTracks>(stream_tracks);
        data.insert::<OnlineTwitch>(HashSet::new());
        if let Some(twitch) = twitch {
            data.insert::<Twitch>(twitch);
        }
        data.insert::<Guilds>(guilds);
        data.insert::<BgGames>(HashMap::new());
        data.insert::<BgVerified>(verified_users);
    }

    // Boot it all up
    if let Err(why) = discord.start().await {
        panic!("Client error: {}", why)
    }
}

#[hook]
async fn before(ctx: &Context, msg: &Message, cmd_name: &str) -> bool {
    let location = match msg.guild(ctx).await {
        Some(guild) => {
            let guild_name = &guild.name;
            let channel_name = if let Channel::Guild(channel) = msg.channel(ctx).await.unwrap() {
                channel.name
            } else {
                panic!("Found non-Guild channel of msg despite msg being in a guild");
            };
            format!("{}:{}", guild_name, channel_name)
        }
        None => "Private".to_owned(),
    };
    info!("[{}] {}: {}", location, msg.author.name, msg.content);
    match ctx.data.write().await.get_mut::<CommandCounter>() {
        Some(counter) => *counter.entry(cmd_name.to_owned()).or_insert(0) += 1,
        None => warn!("Could not get CommandCounter"),
    }
    let _ = msg.channel_id.broadcast_typing(ctx).await;
    true
}

#[hook]
async fn after(_ctx: &Context, _msg: &Message, cmd_name: &str, cmd_result: CommandResult) {
    match cmd_result {
        Ok(()) => info!("Processed command '{}'", cmd_name),
        Err(why) => error!("Command '{}' returned error {:?}", cmd_name, why),
    }
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    let response = match error {
        DispatchError::Ratelimited(seconds) => msg
            .channel_id
            .say(
                ctx,
                format!("Command on cooldown, try again in {} seconds", seconds),
            )
            .await
            .ok(),
        DispatchError::CheckFailed(name, _) => {
            if name == "BgVerified" {
                msg.channel_id
                    .say(
                        ctx,
                        "Only handselected people can use this command.\n\
                        Ask bade to add you if you want to help out tagging backgrounds.",
                    )
                    .await
                    .ok()
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(response) = response {
        response.reaction_delete(ctx, msg.author.id).await;
    }
}

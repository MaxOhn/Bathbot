mod commands;
mod embeds;
pub mod util;
#[macro_use]
mod macros;
mod arguments;
pub mod database;
mod events;
pub mod roppai;
mod scraper;
mod streams;
pub mod structs;

#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;

use crate::scraper::Scraper;
use commands::{fun::*, messages_fun::*, osu::*, streams::*, utility::*};
use database::MySQL;
use events::Handler;
use streams::Twitch;
use structs::Osu;
use structs::*;
pub use util::{discord::get_member, Error};

use chrono::Utc;
use dotenv;
use log::{error, info};
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

pub const WITH_STREAM_TRACK: bool = false;
pub const WITH_SCRAPER: bool = false;
pub const WITH_CUSTOM_EVENTS: bool = false;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Could not load .env file");
    env_logger::init();

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
    let twitch = Twitch::new(&twitch_client_id, &twitch_token)
        .unwrap_or_else(|why| panic!("Could not create Twitch: {}", why));

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

    // ---------------
    // Framework setup
    // ---------------

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefixes(vec!["<", "!!"])
                .owners(owners)
                .delimiter(' ')
                .case_insensitivity(true)
                .ignore_bots(true)
                .no_dm_prefix(true)
        })
        .before(before)
        .after(after)
        .on_dispatch_error(dispatch_error)
        .bucket("songs", |b| b.delay(20).time_span(20).limit(1))
        .await
        .help(&HELP)
        .group(&OSUGENERAL_GROUP)
        .group(&OSU_GROUP)
        .group(&MANIA_GROUP)
        .group(&TAIKO_GROUP)
        .group(&CATCHTHEBEAT_GROUP)
        .group(&FUN_GROUP)
        .group(&MESSAGESFUN_GROUP)
        .group(&UTILITY_GROUP)
        .group(&STREAMTRACKING_GROUP);

    let mut discord = Client::new_with_framework(&discord_token, Handler, framework)
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
        data.insert::<Twitch>(twitch);
        data.insert::<Guilds>(guilds);
        data.insert::<BgGames>(HashMap::new());
    }

    // Boot it all up
    if let Err(why) = discord.start().await {
        panic!("Client error: {}", why)
    }
}

#[hook]
async fn before(ctx: &mut Context, msg: &Message, cmd_name: &str) -> bool {
    let location = match msg.guild(&ctx).await {
        Some(guild) => {
            let guild_name = guild.read().await.name.clone();
            let channel_name = if let Channel::Guild(channel) = msg.channel(&ctx).await.unwrap() {
                channel.read().await.name.clone()
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
    let _ = msg.channel_id.broadcast_typing(&ctx.http);
    true
}

#[hook]
async fn after(_ctx: &mut Context, _msg: &Message, cmd_name: &str, cmd_result: CommandResult) {
    match cmd_result {
        Ok(()) => info!("Processed command '{}'", cmd_name),
        Err(why) => error!("Command '{}' returned error {:?}", cmd_name, why),
    }
}

#[hook]
async fn dispatch_error(ctx: &mut Context, msg: &Message, error: DispatchError) -> () {
    if let DispatchError::Ratelimited(seconds) = error {
        let _ = msg
            .channel_id
            .say(
                &ctx.http,
                &format!("Command on cooldown, try again in {} seconds", seconds),
            )
            .await;
    };
}

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
use commands::{fun::*, osu::*, streams::*, utility::*};
use database::MySQL;
use events::Handler;
use streams::Twitch;
use structs::Osu;
use structs::*;
pub use util::{discord::get_member, globals::MSG_MEMORY, Error};

use chrono::Utc;
use dotenv;
use hey_listen::sync::ParallelDispatcher as Dispatcher;
use log::{error, info};
use rosu::backend::Osu as OsuClient;
use serenity::{
    framework::{standard::DispatchError, StandardFramework},
    model::channel::Channel,
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    sync::Arc,
};
use tokio::runtime::Runtime;
use white_rabbit::Scheduler;

pub const WITH_STREAM_TRACK: bool = false;
pub const WITH_SCRAPER: bool = true;
pub const WITH_CUSTOM_EVENTS: bool = false;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv()?;
    env_logger::init();

    // -----------------
    // Data preparations
    // -----------------

    // Discord
    let discord_token = env::var("DISCORD_TOKEN")?;
    let mut discord = Client::new(&discord_token, Handler)?;

    // Database
    let database_url = env::var("DATABASE_URL")?;
    let mysql = MySQL::new(&database_url)?;

    // Osu
    let osu_token = env::var("OSU_TOKEN")?;
    let osu = OsuClient::new(osu_token);
    let discord_links = mysql.get_discord_links()?;

    // Scraper
    let mut rt = Runtime::new().expect("Could not create runtime");
    let scraper = rt.block_on(Scraper::new())?;

    // Stream tracking
    let twitch_users = mysql.get_twitch_users()?;
    let stream_tracks = mysql.get_stream_tracks()?;
    let twitch_client_id = env::var("TWITCH_CLIENT_ID")?;
    let twitch_token = env::var("TWITCH_TOKEN")?;
    let twitch = Twitch::new(&twitch_client_id, &twitch_token)?;

    // Individual guild settings
    let guilds = mysql.get_guilds()?;

    // General
    let owners = match discord.cache_and_http.http.get_current_application_info() {
        Ok(info) => {
            let mut set = HashSet::new();
            set.insert(info.owner.id);
            set
        }
        Err(why) => {
            return Err(Box::new(Error::Custom(format!(
                "Couldn't get application info: {:?}",
                why
            ))))
        }
    };
    let scheduler = Scheduler::new(4);
    let now = Utc::now();
    let mut dispatcher: Dispatcher<DispatchEvent> = Dispatcher::default();
    dispatcher
        .num_threads(4)
        .expect("Could not construct threadpool");

    // Insert everything
    {
        let mut data = discord.data.write();
        data.insert::<CommandCounter>(HashMap::default());
        data.insert::<Osu>(osu);
        data.insert::<Scraper>(scraper);
        data.insert::<MySQL>(mysql);
        data.insert::<DiscordLinks>(discord_links);
        data.insert::<BootTime>(now);
        data.insert::<PerformanceCalculatorLock>(Arc::new(Mutex::new(())));
        data.insert::<SchedulerKey>(Arc::new(RwLock::new(scheduler)));
        data.insert::<TwitchUsers>(twitch_users);
        data.insert::<StreamTracks>(stream_tracks);
        data.insert::<OnlineTwitch>(HashSet::new());
        data.insert::<Twitch>(twitch);
        data.insert::<ResponseOwner>((
            VecDeque::with_capacity(MSG_MEMORY),
            HashMap::with_capacity(MSG_MEMORY),
        ));
        data.insert::<Guilds>(guilds);
        data.insert::<DispatcherKey>(Arc::new(RwLock::new(dispatcher)));
        data.insert::<BgGameKey>(HashMap::new());
    }

    // ---------------
    // Framework setup
    // ---------------

    discord.with_framework(
        StandardFramework::new()
            .configure(|c| {
                c.prefixes(vec!["<", "!!"])
                    .owners(owners)
                    .delimiter(' ')
                    .case_insensitivity(true)
                    .ignore_bots(true)
                    .no_dm_prefix(true)
            })
            .on_dispatch_error(|ctx, msg, error| {
                if let DispatchError::Ratelimited(seconds) = error {
                    let _ = msg.channel_id.say(
                        &ctx.http,
                        &format!("Command on cooldown, try again in {} seconds", seconds),
                    );
                }
            })
            .help(&HELP)
            .group(&OSUGENERAL_GROUP)
            .group(&OSU_GROUP)
            .group(&MANIA_GROUP)
            .group(&TAIKO_GROUP)
            .group(&CATCHTHEBEAT_GROUP)
            .group(&FUN_GROUP)
            .group(&UTILITY_GROUP)
            .group(&STREAMTRACKING_GROUP)
            .bucket("songs", |b| b.delay(20).time_span(20).limit(1))
            .before(|ctx, msg, cmd_name| {
                let location = match msg.guild(&ctx) {
                    Some(guild) => {
                        let guild_name = guild.read().name.clone();
                        let channel_name = if let Channel::Guild(channel) =
                            msg.channel(&ctx).unwrap()
                        {
                            channel.read().name.clone()
                        } else {
                            panic!("Found non-Guild channel of msg despite msg being in a guild");
                        };
                        format!("{}:{}", guild_name, channel_name)
                    }
                    None => "Private".to_owned(),
                };
                info!("[{}] {}: {}", location, msg.author.name, msg.content);
                match ctx.data.write().get_mut::<CommandCounter>() {
                    Some(counter) => *counter.entry(cmd_name.to_owned()).or_insert(0) += 1,
                    None => error!("Could not get CommandCounter"),
                }
                let _ = msg.channel_id.broadcast_typing(&ctx.http);
                true
            })
            .after(|_, _, cmd_name, error| match error {
                Ok(()) => info!("Processed command '{}'", cmd_name),
                Err(why) => error!("Command '{}' returned error {:?}", cmd_name, why),
            }),
    );
    discord.start()?;
    Ok(())
}

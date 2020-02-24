mod commands;
mod messages;
mod util;
#[macro_use]
mod macros;
mod database;
mod scraper;

#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;

use crate::scraper::Scraper;
use commands::{fun::*, osu::*, streams::*, utility::*};
pub use database::MySQL;
pub use util::Error;

use chrono::{DateTime, Utc};
use log::{error, info};
use rosu::backend::Osu as OsuClient;
use serenity::{
    framework::{standard::DispatchError, StandardFramework},
    model::{channel::Channel, event::ResumedEvent, gateway::Ready},
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Arc,
};
use white_rabbit::Scheduler;

fn setup() -> Result<(String, String, String), Error> {
    kankyo::load()?;
    env_logger::init();
    Ok((
        env::var("DISCORD_TOKEN")?,
        env::var("OSU_TOKEN")?,
        env::var("DATABASE_URL")?,
    ))
}

fn main() -> Result<(), Error> {
    let (discord_token, osu_token, database_url) = setup()?;
    let osu = OsuClient::new(osu_token);
    let mut rt = tokio::runtime::Runtime::new().expect("Could not create runtime");
    let scraper = rt.block_on(Scraper::new())?;
    let mut discord = Client::new(&discord_token, Handler)?;
    let mysql = MySQL::new(&database_url)?;
    let discord_links = mysql.get_discord_links()?;
    let owners = match discord.cache_and_http.http.get_current_application_info() {
        Ok(info) => {
            let mut set = HashSet::new();
            set.insert(info.owner.id);
            set
        }
        Err(why) => {
            return Err(Error::Custom(format!(
                "Couldn't get application info: {:?}",
                why
            )))
        }
    };
    let scheduler = Scheduler::new(4);
    let now = Utc::now();
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
    }
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
            .group(&STREAMS_GROUP)
            .group(&FUN_GROUP)
            .group(&UTILITY_GROUP)
            .bucket("two_per_thirty_cooldown", |b| {
                b.delay(5).time_span(30).limit(2)
            })
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
                info!("[{}] {}: {}", location, msg.author.name, msg.content,);
                match ctx.data.write().get_mut::<CommandCounter>() {
                    Some(counter) => *counter.entry(cmd_name.to_owned()).or_insert(0) += 1,
                    None => error!("Expected CommandCounter in ShareMap."),
                }
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

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed connection");
    }
}

pub struct CommandCounter;

impl TypeMapKey for CommandCounter {
    type Value = HashMap<String, u32>;
}

pub struct Osu;

impl TypeMapKey for Osu {
    type Value = OsuClient;
}

impl TypeMapKey for Scraper {
    type Value = Scraper;
}

impl TypeMapKey for MySQL {
    type Value = MySQL;
}

pub struct DiscordLinks;

impl TypeMapKey for DiscordLinks {
    type Value = HashMap<u64, String>;
}

pub struct BootTime;

impl TypeMapKey for BootTime {
    type Value = DateTime<Utc>;
}

pub struct PerformanceCalculatorLock;

impl TypeMapKey for PerformanceCalculatorLock {
    type Value = Arc<Mutex<()>>;
}

pub struct SchedulerKey;
impl TypeMapKey for SchedulerKey {
    type Value = Arc<RwLock<Scheduler>>;
}

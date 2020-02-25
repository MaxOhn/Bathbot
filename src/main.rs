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
    model::{
        channel::{Channel, Reaction},
        event::ResumedEvent,
        gateway::Ready,
        guild::Guild,
        id::{ChannelId, GuildId, MessageId, RoleId},
        voice::VoiceState,
    },
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
    // -----------------
    // Data preparations
    // -----------------

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
                    None => error!("Could not get CommandCounter"),
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

// --------------
// Event handling
// --------------

struct Handler;
impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed connection");
    }

    fn guild_create(&self, _: Context, guild: Guild, is_new: bool) {
        if is_new {
            info!("'guild_create' triggered for new server '{}'", guild.name);
        }
    }

    fn voice_state_update(
        &self,
        _ctx: Context,
        _guild: Option<GuildId>,
        _old: Option<VoiceState>,
        _new: VoiceState,
    ) {
        // TODO
    }

    fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        let reaction_tracker: HashMap<_, _> = match ctx.data.read().get::<MySQL>() {
            Some(mysql) => mysql
                .get_role_assigns()
                .expect("Could not get role assigns")
                .into_iter()
                .map(|((c, m), r)| ((ChannelId(c), MessageId(m)), RoleId(r)))
                .collect(),
            None => panic!("Could not get MySQL"),
        };
        {
            let mut data = ctx.data.write();
            data.insert::<ReactionTracker>(reaction_tracker);
        }
    }

    fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        let key = (reaction.channel_id, reaction.message_id);
        let role: Option<RoleId> = match ctx.data.read().get::<ReactionTracker>() {
            Some(tracker) => {
                if tracker.contains_key(&key) {
                    Some(*tracker.get(&key).unwrap())
                } else {
                    None
                }
            }
            None => {
                error!("Could not get ReactionTracker");
                return;
            }
        };
        if let Some(role) = role {
            let channel = match reaction.channel(&ctx) {
                Ok(channel) => channel,
                Err(why) => {
                    error!("Could not get Channel from reaction: {}", why);
                    return;
                }
            };
            let guild_lock = match channel.guild() {
                Some(guild_channel) => match guild_channel.read().guild(&ctx) {
                    Some(guild) => guild.clone(),
                    None => {
                        error!("Could not get Guild from reaction");
                        return;
                    }
                },
                None => {
                    error!("Could not get GuildChannel from reaction");
                    return;
                }
            };
            let guild = guild_lock.read();
            let mut member = match guild.member(&ctx, reaction.user_id) {
                Ok(member) => member,
                Err(why) => {
                    error!("Could not get Member from reaction: {}", why);
                    return;
                }
            };
            let role_name = role
                .to_role_cached(&ctx.cache)
                .expect("Role not found in cache")
                .name;
            if let Err(why) = member.add_role(&ctx.http, role) {
                error!("Could not add role to member for reaction: {}", why);
            } else {
                info!(
                    "Assigned role '{}' to member {}",
                    role_name,
                    member.user.read().name
                );
            }
        }
    }

    fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        let key = (reaction.channel_id, reaction.message_id);
        let role = match ctx.data.read().get::<ReactionTracker>() {
            Some(tracker) => {
                if tracker.contains_key(&key) {
                    Some(*tracker.get(&key).unwrap())
                } else {
                    None
                }
            }
            None => {
                error!("Could not get ReactionTracker");
                return;
            }
        };
        if let Some(role) = role {
            let channel = match reaction.channel(&ctx) {
                Ok(channel) => channel,
                Err(why) => {
                    error!("Could not get Channel from reaction: {}", why);
                    return;
                }
            };
            let guild_lock = match channel.guild() {
                Some(guild_channel) => match guild_channel.read().guild(&ctx) {
                    Some(guild) => guild.clone(),
                    None => {
                        error!("Could not get Guild from reaction");
                        return;
                    }
                },
                None => {
                    error!("Could not get GuildChannel from reaction");
                    return;
                }
            };
            let guild = guild_lock.read();
            let mut member = match guild.member(&ctx, reaction.user_id) {
                Ok(member) => member,
                Err(why) => {
                    error!("Could not get Member from reaction: {}", why);
                    return;
                }
            };
            let role_name = role
                .to_role_cached(&ctx.cache)
                .expect("Role not found in cache")
                .name;
            if let Err(why) = member.remove_role(&ctx.http, role) {
                error!("Could not remove role from member for reaction: {}", why);
            } else {
                info!(
                    "Removed role '{}' from member {}",
                    role_name,
                    member.user.read().name
                );
            }
        }
    }
}

// ------------------
// Struct definitions
// ------------------

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

pub struct ReactionTracker;
impl TypeMapKey for ReactionTracker {
    type Value = HashMap<(ChannelId, MessageId), RoleId>;
}

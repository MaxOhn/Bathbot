mod commands;
mod messages;
pub mod util;
#[macro_use]
mod macros;
pub mod database;
mod scraper;
mod streams;

#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;

use crate::scraper::Scraper;
use commands::{fun::*, osu::*, streams::*, utility::*};
use database::{Guild as GuildDB, MySQL, Platform, StreamTrack};
use messages::BasicEmbedData;
use streams::{Twitch, TwitchStream};
pub use util::{discord::get_member, globals::MSG_MEMORY, Error};

use chrono::{DateTime, Utc};
use log::{error, info};
use rosu::backend::Osu as OsuClient;
use serenity::{
    framework::{standard::DispatchError, StandardFramework},
    model::{
        channel::{Channel, Reaction, ReactionType},
        event::ResumedEvent,
        gateway::Ready,
        guild::{Guild, Member},
        id::{ChannelId, GuildId, MessageId, RoleId, UserId},
        voice::VoiceState,
    },
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    sync::Arc,
};
use strfmt::strfmt;
use tokio::runtime::Runtime;
use white_rabbit::{DateResult, Duration, Scheduler, Utc as UtcWR};

pub const WITH_STREAM_TRACK: bool = false;
pub const WITH_SCRAPER: bool = false;

fn setup() -> Result<(), Error> {
    kankyo::load()?;
    env_logger::init();
    Ok(())
}

fn main() -> Result<(), Error> {
    setup()?;
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
            return Err(Error::Custom(format!(
                "Couldn't get application info: {:?}",
                why
            )))
        }
    };
    let scheduler = Scheduler::new(4);
    let now = Utc::now();

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
            .group(&STREAMTRACKING_GROUP)
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

    fn guild_create(&self, ctx: Context, guild: Guild, is_new: bool) {
        if is_new {
            let guild = {
                let data = ctx.data.read();
                let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                match mysql.insert_guild(guild.id.0) {
                    Ok(g) => {
                        info!("Inserted new guild {} to database", guild.name);
                        Some(g)
                    }
                    Err(why) => {
                        error!(
                            "Could not insert new guild '{}' to database: {}",
                            guild.name, why
                        );
                        None
                    }
                }
            };
            if let Some(guild) = guild {
                let mut data = ctx.data.write();
                let guilds = data.get_mut::<Guilds>().expect("Could not get Guilds");
                guilds.insert(guild.guild_id, guild);
            }
        }
    }

    fn voice_state_update(
        &self,
        ctx: Context,
        guild: Option<GuildId>,
        old: Option<VoiceState>,
        new: VoiceState,
    ) {
        // Try assigning the server's VC role to the member
        if let Some(guild_id) = guild {
            let role = {
                let data = ctx.data.read();
                data.get::<Guilds>()
                    .and_then(|guilds| guilds.get(&guild_id))
                    .and_then(|guild| guild.vc_role)
            };
            // If the server has configured such a role
            if let Some(role) = role {
                // Get the event's member
                let mut member = match guild_id.member(&ctx, new.user_id) {
                    Ok(member) => member,
                    Err(why) => {
                        warn!("Could not get member for VC update: {}", why);
                        return;
                    }
                };
                let role_name = role
                    .to_role_cached(&ctx.cache)
                    .expect("Role not found in cache")
                    .name;
                // If either the member left VC, or joined the afk channel
                let remove_role = new.channel_id.map_or(true, |channel| {
                    channel
                        .name(&ctx.cache)
                        .map_or(false, |name| &name.to_lowercase() == "afk")
                });
                if remove_role {
                    // Remove role
                    if let Err(why) = member.remove_role(&ctx.http, role) {
                        error!("Could not remove role from member for VC update: {}", why);
                    } else {
                        info!(
                            "Removed role '{}' from member {}",
                            role_name,
                            member.user.read().name
                        );
                    }
                } else {
                    // Add role if the member is either coming from the afk channel
                    // or hasn't been in a VC before
                    let add_role = old.map_or(true, |old_state| {
                        old_state
                            .channel_id
                            .unwrap()
                            .name(&ctx.cache)
                            .map_or(true, |name| &name.to_lowercase() == "afk")
                    });
                    if add_role {
                        if let Err(why) = member.add_role(&ctx.http, role) {
                            error!("Could not add role to member for VC update: {}", why);
                        } else {
                            info!(
                                "Assigned role '{}' to member {}",
                                role_name,
                                member.user.read().name
                            );
                        }
                    }
                }
            }
        }
    }

    fn guild_member_addition(&self, _ctx: Context, _guild_id: GuildId, _new_member: Member) {
        // TODO
    }

    fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        // Tracking streams
        if WITH_STREAM_TRACK {
            let track_delay = 10;
            let scheduler = {
                let mut data = ctx.data.write();
                data.get_mut::<SchedulerKey>()
                    .expect("Could not get SchedulerKey")
                    .clone()
            };
            let mut scheduler = scheduler.write();
            let http = ctx.http.clone();
            let data = ctx.data.clone();
            scheduler.add_task_duration(Duration::seconds(track_delay), move |_| {
                //debug!("Checking stream tracks...");
                let now_online = {
                    let reading = data.read();

                    // Get data about what needs to be tracked for which channel
                    let stream_tracks = reading
                        .get::<StreamTracks>()
                        .expect("Could not get StreamTracks");
                    let user_ids: Vec<_> = stream_tracks
                        .iter()
                        .filter(|track| track.platform == Platform::Twitch)
                        .map(|track| track.user_id)
                        .collect();
                    // Twitch provides up to 100 streams per request, otherwise its trimmed
                    if user_ids.len() > 100 {
                        warn!("Reached 100 twitch trackings, improve handling!");
                    }

                    // Get stream data about all streams that need to be tracked
                    let twitch = reading.get::<Twitch>().expect("Could not get Twitch");
                    let mut rt = Runtime::new().expect("Could not create runtime for streams");
                    let mut streams = match rt.block_on(twitch.get_streams(&user_ids)) {
                        Ok(streams) => streams,
                        Err(why) => {
                            warn!("Error while retrieving streams: {}", why);
                            return DateResult::Repeat(
                                UtcWR::now() + Duration::minutes(track_delay),
                            );
                        }
                    };

                    // Filter streams whether they're live
                    streams.retain(TwitchStream::is_live);
                    let online_streams = reading
                        .get::<OnlineTwitch>()
                        .expect("Could not get OnlineTwitch");
                    let now_online: HashSet<_> =
                        streams.iter().map(|stream| stream.user_id).collect();

                    // If there was no activity change since last time, don't do anything
                    if &now_online == online_streams {
                        //debug!("No activity change");
                        None
                    } else {
                        // Filter streams whether its already known they're live
                        streams.retain(|stream| !online_streams.contains(&stream.user_id));
                        let mut fmt_data = HashMap::new();
                        fmt_data.insert(String::from("width"), String::from("360"));
                        fmt_data.insert(String::from("height"), String::from("180"));

                        // Put streams into a more suitable data type and process the thumbnail url
                        let streams: HashMap<u64, TwitchStream> = streams
                            .into_iter()
                            .map(|mut stream| {
                                if let Ok(thumbnail) = strfmt(&stream.thumbnail_url, &fmt_data) {
                                    stream.thumbnail_url = thumbnail;
                                }
                                (stream.user_id, stream)
                            })
                            .collect();

                        // Process each tracking by notifying corresponding channels
                        for track in stream_tracks {
                            if streams.contains_key(&track.user_id) {
                                let stream = streams.get(&track.user_id).unwrap();
                                let data = BasicEmbedData::create_twitch_stream_notif(stream);
                                let _ = ChannelId(track.channel_id)
                                    .send_message(&http, |m| m.embed(|e| data.build(e)));
                            }
                        }
                        Some(now_online)
                    }
                };
                if let Some(now_online) = now_online {
                    let mut writing = data.write();
                    let online_twitch = writing
                        .get_mut::<OnlineTwitch>()
                        .expect("Could not get OnlineTwitch");
                    online_twitch.clear();
                    for id in now_online {
                        online_twitch.insert(id);
                    }
                }
                //debug!("Stream track check done");
                DateResult::Repeat(UtcWR::now() + Duration::minutes(track_delay))
            });
            info!("Stream tracking started");
        } else {
            info!("Stream tracking skipped");
        }

        // Tracking reactions
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
        // Check if the reacting user wants a bot response to be deleted
        if let ReactionType::Unicode(emote) = &reaction.emoji {
            if emote.as_str() == "❌" {
                let data = ctx.data.read();
                let (_, owners) = data
                    .get::<ResponseOwner>()
                    .expect("Could not get ResponseOwner");
                let is_owner = owners
                    .get(&reaction.message_id)
                    .map_or(false, |owner| owner == &reaction.user_id);
                if is_owner {
                    if let Err(why) = reaction
                        .channel_id
                        .delete_message(&ctx.http, reaction.message_id)
                    {
                        warn!("Could not delete message after owner's reaction: {}", why);
                    } else {
                        info!("Deleted message upon owner's reaction");
                    }
                }
            }
        }
        // Check if the reacting user now gets a role
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
            if let Some(mut member) = get_member(&ctx, reaction.channel_id, reaction.user_id) {
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
    }

    fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        // Check if the reacting user now loses a role
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
            if let Some(mut member) = get_member(&ctx, reaction.channel_id, reaction.user_id) {
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

pub struct TwitchUsers;
impl TypeMapKey for TwitchUsers {
    type Value = HashMap<String, u64>;
}

pub struct StreamTracks;
impl TypeMapKey for StreamTracks {
    type Value = HashSet<StreamTrack>;
}

pub struct OnlineTwitch;
impl TypeMapKey for OnlineTwitch {
    type Value = HashSet<u64>;
}

impl TypeMapKey for Twitch {
    type Value = Twitch;
}

pub struct ResponseOwner;
impl TypeMapKey for ResponseOwner {
    type Value = (VecDeque<MessageId>, HashMap<MessageId, UserId>);
}

pub struct Guilds;
impl TypeMapKey for Guilds {
    type Value = HashMap<GuildId, GuildDB>;
}

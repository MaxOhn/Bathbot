use crate::{
    database::{MySQL, Platform},
    embeds::BasicEmbedData,
    scraper::Scraper,
    streams::{Twitch, TwitchStream},
    structs::{Guilds, OnlineTwitch, ReactionTracker, ResponseOwner, SchedulerKey, StreamTracks},
    util::{
        discord::get_member,
        globals::{MAIN_GUILD_ID, TOP_ROLE_ID, UNCHECKED_ROLE_ID, WELCOME_CHANNEL},
    },
    WITH_CUSTOM_EVENTS, WITH_STREAM_TRACK,
};

use chrono::{Duration as ChronoDur, Utc};
use log::{error, info};
use rosu::models::GameMode;
use serenity::{
    model::{
        channel::{Reaction, ReactionType},
        event::ResumedEvent,
        gateway::{Activity, Ready},
        guild::{Guild, Member},
        id::{ChannelId, GuildId, MessageId, RoleId},
        voice::VoiceState,
    },
    prelude::*,
};
use std::collections::{HashMap, HashSet};
use strfmt::strfmt;
use tokio::runtime::Runtime;
use white_rabbit::{DateResult, Duration, Utc as UtcWR};

pub struct Handler;

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        if let Some(shard) = ready.shard {
            info!(
                "{} is connected on shard {}/{}",
                ready.user.name, shard[0], shard[1],
            );
        } else {
            info!("Connected as {}", ready.user.name);
        }
        ctx.set_activity(Activity::playing("osu! (<help)"));
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

    fn guild_member_addition(&self, ctx: Context, guild_id: GuildId, new_member: Member) {
        if guild_id.0 == MAIN_GUILD_ID {
            let data = ctx.data.read();
            let mysql = data.get::<MySQL>().expect("Could not get MySQL");
            if let Err(why) = mysql.insert_unchecked_member(new_member.user_id().0, Utc::now()) {
                error!("Could not insert unchecked member into database: {}", why);
            }
            let _ = ChannelId(WELCOME_CHANNEL).say(
                &ctx.http,
                format!(
                    "{} just joined the server, awaiting approval",
                    new_member.mention()
                ),
            );
        }
    }

    fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        // Custom events
        if WITH_CUSTOM_EVENTS {
            let track_delay = 1;
            let day_limit = 10;
            let scheduler = {
                let mut data = ctx.data.write();
                data.get_mut::<SchedulerKey>()
                    .expect("Could not get SchedulerKey")
                    .clone()
            };
            let mut scheduler = scheduler.write();
            let http = ctx.http.clone();
            let data = ctx.data.clone();
            let cache = ctx.cache.clone();
            scheduler.add_task_duration(Duration::minutes(10), move |_| {
                let data = data.read();
                let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                // Handle Not Checked role
                match mysql.get_unchecked_members() {
                    Ok(members) => {
                        let limit_date = Utc::now() - ChronoDur::days(day_limit);
                        let guild_id = GuildId(MAIN_GUILD_ID);
                        for (user_id, join_date) in members {
                            if limit_date > join_date {
                                if let Err(why) = guild_id.kick(&http, user_id) {
                                    warn!(
                                        "Could not kick member {} who joined {}: {}",
                                        user_id, join_date, why
                                    );
                                } else {
                                    info!(
                                        "Kicked member {} for being unchecked for {} days",
                                        user_id, day_limit
                                    );
                                    let member = guild_id
                                        .member((&cache, &*http), user_id)
                                        .ok()
                                        .map_or_else(
                                            || format!("id {}", user_id),
                                            |member| member.mention(),
                                        );
                                    let _ = ChannelId(WELCOME_CHANNEL).say(
                                        &http,
                                        format!(
                                            "Kicking member {} for being unchecked for {} days",
                                            member, day_limit,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    Err(why) => warn!("Could not get unchecked members from database: {}", why),
                }
                // Handle Top role
                let mut rt = Runtime::new().unwrap();
                let scraper = data.get::<Scraper>().expect("Could not get Scraper");
                let (std, tko, ctb, mna) = rt.block_on(async {
                    let std = scraper.get_top50_names("be", GameMode::STD);
                    let tko = scraper.get_top50_names("be", GameMode::TKO);
                    let ctb = scraper.get_top50_names("be", GameMode::CTB);
                    let mna = scraper.get_top50_names("be", GameMode::MNA);
                    (std.await, tko.await, ctb.await, mna.await)
                });
                // Top 10 std
                let mut all = std.map_or_else(
                    |why| {
                        warn!("Could not get top 50 for std: {}", why);
                        Vec::new()
                    },
                    |m| m.into_iter().take(10).collect(),
                );
                // Top 3 tko
                let mut tko = tko.map_or_else(
                    |why| {
                        warn!("Could not get top 50 for tko: {}", why);
                        Vec::new()
                    },
                    |m| m.into_iter().take(3).collect(),
                );
                // Top 3 ctb
                let mut ctb = ctb.map_or_else(
                    |why| {
                        warn!("Could not get top 50 for ctb: {}", why);
                        Vec::new()
                    },
                    |m| m.into_iter().take(3).collect(),
                );
                // Top 5 mna
                let mut mna = mna.map_or_else(
                    |why| {
                        warn!("Could not get top 50 for mna: {}", why);
                        Vec::new()
                    },
                    |m| m.into_iter().take(5).collect(),
                );
                all.append(&mut tko);
                all.append(&mut ctb);
                all.append(&mut mna);
                match mysql.get_manual_links() {
                    Ok(links) => {
                        let guild_id = GuildId(MAIN_GUILD_ID);
                        let role = RoleId(TOP_ROLE_ID);
                        let members = guild_id.members(&http, None, None).unwrap_or_else(|why| {
                            warn!("Could not get guild members for top role: {}", why);
                            Vec::new()
                        });
                        // Check all guild's members
                        for mut member in members {
                            let name = links.get(&member.user_id().0);
                            // If name is contained in manual links
                            if let Some(osu_name) = name {
                                // If member already has top role, check if it remains
                                if member.roles.contains(&role) {
                                    if !all.contains(&osu_name) {
                                        if let Err(why) = member.remove_role(&http, role) {
                                            error!(
                                                "Could not remove top role from member: {}",
                                                why
                                            );
                                        } else {
                                            info!(
                                                "Removed 'Top' role from member {}",
                                                member.user.read().name
                                            );
                                        }
                                    }
                                // Member does not have top role yet, 'all' contains the name
                                } else if all.contains(&osu_name) {
                                    if let Err(why) = member.add_role(&http, role) {
                                        error!("Could not add top role to member: {}", why);
                                    } else {
                                        info!(
                                            "Added 'Top' role to member {}",
                                            member.user.read().name
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(why) => warn!("Could not get manual links from database: {}", why),
                }
                DateResult::Repeat(UtcWR::now() + Duration::days(track_delay))
            });
        }

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
                        None
                    } else {
                        // Filter streams whether its already known they're live
                        streams.retain(|stream| !online_streams.contains(&stream.user_id));

                        let ids: Vec<_> = streams.iter().map(|s| s.user_id).collect();
                        let users: HashMap<_, _> = match rt.block_on(twitch.get_users(&ids)) {
                            Ok(users) => users.into_iter().map(|u| (u.user_id, u)).collect(),
                            Err(why) => {
                                warn!("Error while retrieving twitch users: {}", why);
                                return DateResult::Repeat(
                                    UtcWR::now() + Duration::minutes(track_delay),
                                );
                            }
                        };

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
                                let data = BasicEmbedData::create_twitch_stream_notif(
                                    stream,
                                    users.get(&stream.user_id).unwrap(),
                                );
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
                        let name = reaction.channel_id.name(&ctx.cache);
                        let user = reaction.user_id.to_user((&ctx.cache, &*ctx.http));
                        if let Ok(user) = user {
                            if let Some(channel_name) = name {
                                info!(
                                    "Deleted message from {} in channel {} after their reaction",
                                    user.name, channel_name
                                );
                            }
                        } else {
                            info!("Deleted message upon owner's reaction");
                        }
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

    fn guild_member_update(&self, ctx: Context, old_if_available: Option<Member>, new: Member) {
        // If member loses the "Not checked" role, they gets removed from
        // unchecked_members database table and greeted in #general chat
        if new.guild_id.0 == MAIN_GUILD_ID {
            if let Some(old) = old_if_available {
                // Member lost a role
                if new.roles.len() < old.roles.len() {
                    // Get the lost role
                    let role = old
                        .roles
                        .iter()
                        .find(|role| !new.roles.contains(role))
                        .map(|id| id.0 == UNCHECKED_ROLE_ID);
                    // Is it the right role?
                    if let Some(true) = role {
                        let data = ctx.data.read();
                        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                        // Mark user as checked by removing him from unchecked database
                        if let Err(why) = mysql.remove_unchecked_member(new.user_id().0) {
                            warn!("Could not remove unchecked member from database: {}", why);
                        } else {
                            info!("Member {} lost the 'Not checked' role", new.display_name());
                            let _ = ChannelId(MAIN_GUILD_ID).say(
                                &ctx.http,
                                format!("welcome {}, enjoy ur stay o/", new.display_name()),
                            );
                        }
                    }
                }
            }
        }
    }
}

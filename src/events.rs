use crate::{
    database::{InsertableMessage, MySQL, Platform},
    embeds::BasicEmbedData,
    scraper::Scraper,
    streams::{Twitch, TwitchStream},
    structs::{Guilds, OnlineTwitch, ReactionTracker, StreamTracks},
    util::{
        discord::get_member,
        globals::{MAIN_GUILD_ID, TOP_ROLE_ID, UNCHECKED_ROLE_ID, WELCOME_CHANNEL},
    },
    WITH_CUSTOM_EVENTS, WITH_STREAM_TRACK,
};

use chrono::{Duration as ChronoDur, Utc};
use log::{error, info};
use rayon::prelude::*;
use rosu::models::GameMode;
use serenity::{
    async_trait,
    http::Http,
    model::{
        channel::{Message, Reaction},
        event::ResumedEvent,
        gateway::{Activity, Ready},
        guild::{Guild, Member},
        id::{ChannelId, GuildId, MessageId, RoleId},
        misc::Mentionable,
        voice::VoiceState,
    },
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use strfmt::strfmt;
use tokio::{task, time};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        // Custom events
        if WITH_CUSTOM_EVENTS {
            let http = ctx.http.clone();
            let data = ctx.data.clone();
            let _ = task::spawn(async move {
                let http = http;
                let data = data;
                let track_delay = 1;
                let day_limit = 10;
                let mut interval = time::interval(time::Duration::from_secs(track_delay * 86_400));
                loop {
                    _not_checked_role(&http, Arc::clone(&data), day_limit).await;
                    _top_role(&http, Arc::clone(&data)).await;
                    info!("Handled unchecked members and top role distribution");
                    interval.tick().await;
                }
            });
        }

        // Tracking streams
        if WITH_STREAM_TRACK {
            let http = ctx.http.clone();
            let data = ctx.data.clone();
            let _ = task::spawn(async {
                let http = http;
                let data = data;
                let track_delay = 10;
                let mut interval = time::interval(time::Duration::from_secs(track_delay * 60));
                loop {
                    _check_streams(&http, Arc::clone(&data).clone()).await;
                    interval.tick().await;
                }
            });
            info!("Stream tracking started");
        } else {
            info!("Stream tracking skipped");
        }

        // Tracking reactions
        {
            let mut data = ctx.data.write().await;
            match data.get::<MySQL>() {
                Some(mysql) => {
                    let reaction_tracker: HashMap<_, _> = mysql
                        .get_role_assigns()
                        .expect("Could not get role assigns")
                        .into_iter()
                        .map(|((c, m), r)| ((ChannelId(c), MessageId(m)), RoleId(r)))
                        .collect();
                    {
                        data.insert::<ReactionTracker>(reaction_tracker);
                    }
                }
                None => warn!("Could not get MySQL for reaction_tracker"),
            }
        }

        if let Some(shard) = ready.shard {
            info!(
                "{} is connected on shard {}/{}",
                ready.user.name, shard[0], shard[1],
            );
        } else {
            info!("Connected as {}", ready.user.name);
        }
        ctx.set_activity(Activity::playing("osu! (<help)")).await;
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed connection");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // Message saving
        if !(msg.content.is_empty()
            || msg.content.starts_with('<')
            || msg.content.starts_with('!')
            || msg.content.starts_with('>')
            || msg.content.starts_with('&')
            || msg.content.starts_with('$'))
        {
            let msg_vec = vec![InsertableMessage {
                id: msg.id.0,
                channel_id: msg.channel_id.0,
                author: msg.author.id.0,
                content: msg.content,
                timestamp: msg.timestamp.naive_utc(),
            }];
            let data = ctx.data.read().await;
            let with_tracking = msg
                .guild_id
                .and_then(|guild_id| {
                    data.get::<Guilds>()
                        .and_then(|guilds| guilds.get(&guild_id))
                })
                .map(|guild_db| guild_db.message_tracking)
                .unwrap_or_else(|| false);
            if with_tracking {
                let _ = data
                    .get::<MySQL>()
                    .and_then(|mysql| mysql.insert_msgs(&msg_vec).ok());
            }
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: bool) {
        if is_new {
            // Insert basic guild info into database
            let guild = {
                let data = ctx.data.read().await;
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
                let mut data = ctx.data.write().await;
                let guilds = data.get_mut::<Guilds>().expect("Could not get Guilds");
                guilds.insert(guild.guild_id, guild);
            }
        }
    }

    async fn voice_state_update(
        &self,
        ctx: Context,
        guild: Option<GuildId>,
        old: Option<VoiceState>,
        new: VoiceState,
    ) {
        // Try assigning the server's VC role to the member
        if let Some(guild_id) = guild {
            let role = {
                let data = ctx.data.read().await;
                data.get::<Guilds>()
                    .and_then(|guilds| guilds.get(&guild_id))
                    .and_then(|guild| guild.vc_role)
            };
            // If the server has configured such a role
            if let Some(role) = role {
                // Get the event's member
                let mut member = match guild_id.member(&ctx, new.user_id).await {
                    Ok(member) => member,
                    Err(why) => {
                        warn!("Could not get member for VC update: {}", why);
                        return;
                    }
                };
                let role_name = role
                    .to_role_cached(&ctx.cache)
                    .await
                    .expect("Role not found in cache")
                    .name;
                // If either the member left VC, or joined the afk channel
                let remove_role = match new.channel_id {
                    None => true,
                    Some(channel) => channel
                        .name(&ctx.cache)
                        .await
                        .map_or(false, |name| &name.to_lowercase() == "afk"),
                };
                if remove_role {
                    // Remove role
                    if let Err(why) = member.remove_role(&ctx.http, role).await {
                        error!("Could not remove role from member for VC update: {}", why);
                    } else {
                        info!(
                            "Removed role '{}' from member {}",
                            role_name,
                            member.user.read().await.name
                        );
                    }
                } else {
                    // Add role if the member is either coming from the afk channel
                    // or hasn't been in a VC before
                    let add_role = match old {
                        None => true,
                        Some(old_state) => old_state
                            .channel_id
                            .unwrap()
                            .name(&ctx.cache)
                            .await
                            .map_or(true, |name| &name.to_lowercase() == "afk"),
                    };
                    if add_role {
                        if let Err(why) = member.add_role(&ctx.http, role).await {
                            error!("Could not add role to member for VC update: {}", why);
                        } else {
                            info!(
                                "Assigned role '{}' to member {}",
                                role_name,
                                member.user.read().await.name
                            );
                        }
                    }
                }
            }
        }
    }

    async fn guild_member_addition(&self, ctx: Context, guild_id: GuildId, new_member: Member) {
        if guild_id.0 == MAIN_GUILD_ID {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().expect("Could not get MySQL");
            if let Err(why) =
                mysql.insert_unchecked_member(new_member.user_id().await.0, Utc::now())
            {
                error!("Could not insert unchecked member into database: {}", why);
            }
            let _ = ChannelId(WELCOME_CHANNEL).say(
                &ctx.http,
                format!(
                    "{} just joined the server, awaiting approval",
                    new_member.mention().await
                ),
            );
        }
    }

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        // Check if the reacting user now gets a role
        role_assignment(&ctx, &reaction).await;
    }

    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        // Check if the reacting user now loses a role
        let key = (reaction.channel_id, reaction.message_id);
        let role = match ctx.data.read().await.get::<ReactionTracker>() {
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
            if let Some(mut member) = get_member(&ctx, reaction.channel_id, reaction.user_id).await
            {
                let role_name = role
                    .to_role_cached(&ctx.cache)
                    .await
                    .expect("Role not found in cache")
                    .name;
                if let Err(why) = member.remove_role(&ctx.http, role).await {
                    error!("Could not remove role from member for reaction: {}", why);
                } else {
                    info!(
                        "Removed role '{}' from member {}",
                        role_name,
                        member.user.read().await.name
                    );
                }
            }
        }
    }

    async fn guild_member_update(
        &self,
        ctx: Context,
        old_if_available: Option<Member>,
        new: Member,
    ) {
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
                        let data = ctx.data.read().await;
                        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                        // Mark user as checked by removing him from unchecked database
                        if let Err(why) = mysql.remove_unchecked_member(new.user_id().await.0) {
                            warn!("Could not remove unchecked member from database: {}", why);
                        } else {
                            let display_name = new.display_name().await;
                            info!("Member {} lost the 'Not checked' role", display_name);
                            let _ = ChannelId(MAIN_GUILD_ID).say(
                                &ctx.http,
                                format!("welcome {}, enjoy ur stay o/", display_name),
                            );
                        }
                    }
                }
            }
        }
    }
}

async fn _not_checked_role(http: &Http, data: Arc<RwLock<ShareMap>>, day_limit: i64) {
    let data = data.read().await;
    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
    // Handle Not Checked role
    match mysql.get_unchecked_members() {
        Ok(members) => {
            let limit_date = Utc::now() - ChronoDur::days(day_limit);
            let guild_id = GuildId(MAIN_GUILD_ID);
            for (user_id, join_date) in members {
                if limit_date > join_date {
                    if let Err(why) = guild_id.kick(http, user_id).await {
                        warn!(
                            "Could not kick member {} who joined {}: {}",
                            user_id, join_date, why
                        );
                    } else {
                        info!(
                            "Kicked member {} for being unchecked for {} days",
                            user_id, day_limit
                        );
                        let _ = ChannelId(WELCOME_CHANNEL)
                            .say(
                                &http,
                                format!(
                                    "Kicking member {} for being unchecked for {} days",
                                    user_id.mention().await,
                                    day_limit,
                                ),
                            )
                            .await;
                        if let Err(why) = mysql.remove_unchecked_member(user_id.0) {
                            warn!("Error while removing unchecked member from DB: {}", why);
                        }
                    }
                }
            }
        }
        Err(why) => warn!("Could not get unchecked members from database: {}", why),
    }
}

async fn _top_role(http: &Http, data: Arc<RwLock<ShareMap>>) {
    let data = data.read().await;
    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
    // Handle Top role
    let scraper = data.get::<Scraper>().expect("Could not get Scraper");
    // Top 10 std
    let mut all = scraper
        .get_top50_names("be", GameMode::STD)
        .await
        .map_or_else(
            |why| {
                warn!("Could not get top 50 for std: {}", why);
                Vec::new()
            },
            |m| m.into_iter().take(10).collect(),
        );
    // Top 5 mna
    let mna = scraper
        .get_top50_names("be", GameMode::MNA)
        .await
        .map_or_else(
            |why| {
                warn!("Could not get top 50 for mna: {}", why);
                Vec::new()
            },
            |m| m.into_iter().take(5).collect(),
        );
    // Top 3 tko
    let tko = scraper
        .get_top50_names("be", GameMode::TKO)
        .await
        .map_or_else(
            |why| {
                warn!("Could not get top 50 for tko: {}", why);
                Vec::new()
            },
            |m| m.into_iter().take(3).collect(),
        );
    // Top 3 ctb
    let ctb = scraper
        .get_top50_names("be", GameMode::CTB)
        .await
        .map_or_else(
            |why| {
                warn!("Could not get top 50 for ctb: {}", why);
                Vec::new()
            },
            |m| m.into_iter().take(3).collect(),
        );
    all.extend(tko);
    all.extend(ctb);
    all.extend(mna);
    match mysql.get_manual_links() {
        Ok(links) => {
            let guild_id = GuildId(MAIN_GUILD_ID);
            let role = RoleId(TOP_ROLE_ID);
            let members = guild_id
                .members(http, None, None)
                .await
                .unwrap_or_else(|why| {
                    warn!("Could not get guild members for top role: {}", why);
                    Vec::new()
                });
            // Check all guild's members
            for mut member in members {
                let name = links.get(&member.user_id().await.0);
                // If name is contained in manual links
                if let Some(osu_name) = name {
                    // If member already has top role, check if it remains
                    if member.roles.contains(&role) {
                        if !all.contains(&osu_name) {
                            if let Err(why) = member.remove_role(http, role).await {
                                error!("Could not remove top role from member: {}", why);
                            } else {
                                info!(
                                    "Removed 'Top' role from member {}",
                                    member.user.read().await.name
                                );
                            }
                        }
                    // Member does not have top role yet, 'all' contains the name
                    } else if all.contains(&osu_name) {
                        if let Err(why) = member.add_role(http, role).await {
                            error!("Could not add top role to member: {}", why);
                        } else {
                            info!(
                                "Added 'Top' role to member {}",
                                member.user.read().await.name
                            );
                        }
                    }
                }
            }
        }
        Err(why) => warn!("Could not get manual links from database: {}", why),
    }
}

async fn _check_streams(http: &Http, data: Arc<RwLock<ShareMap>>) {
    let now_online = {
        let reading = data.read().await;

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
        let mut streams = match twitch.get_streams(&user_ids).await {
            Ok(streams) => streams,
            Err(why) => {
                warn!("Error while retrieving streams: {}", why);
                return;
            }
        };

        // Filter streams whether they're live
        streams.retain(TwitchStream::is_live);
        let online_streams = reading
            .get::<OnlineTwitch>()
            .expect("Could not get OnlineTwitch");
        let now_online: HashSet<_> = streams.iter().map(|stream| stream.user_id).collect();

        // If there was no activity change since last time, don't do anything
        if &now_online == online_streams {
            None
        } else {
            // Filter streams whether its already known they're live
            streams.retain(|stream| !online_streams.contains(&stream.user_id));

            let ids: Vec<_> = streams.iter().map(|s| s.user_id).collect();
            let users: HashMap<_, _> = match twitch.get_users(&ids).await {
                Ok(users) => users.into_iter().map(|u| (u.user_id, u)).collect(),
                Err(why) => {
                    warn!("Error while retrieving twitch users: {}", why);
                    return;
                }
            };

            let mut fmt_data = HashMap::new();
            fmt_data.insert(String::from("width"), String::from("360"));
            fmt_data.insert(String::from("height"), String::from("180"));

            // Put streams into a more suitable data type and process the thumbnail url
            let streams: HashMap<u64, TwitchStream> = streams
                .into_par_iter()
                .map(|mut stream| {
                    if let Ok(thumbnail) = strfmt(&stream.thumbnail_url, &fmt_data) {
                        stream.thumbnail_url = thumbnail;
                    }
                    (stream.user_id, stream)
                })
                .collect();

            // Process each tracking by notifying corresponding channels
            stream_tracks.par_iter().for_each(|track| {
                if streams.contains_key(&track.user_id) {
                    let stream = streams.get(&track.user_id).unwrap();
                    let data = BasicEmbedData::create_twitch_stream_notif(
                        stream,
                        users.get(&stream.user_id).unwrap(),
                    );
                    let _ = ChannelId(track.channel_id)
                        .send_message(http, |m| m.embed(|e| data.build(e)));
                }
            });
            Some(now_online)
        }
    };
    if let Some(now_online) = now_online {
        let mut writing = data.write().await;
        let online_twitch = writing
            .get_mut::<OnlineTwitch>()
            .expect("Could not get OnlineTwitch");
        online_twitch.clear();
        for id in now_online {
            online_twitch.insert(id);
        }
    }
}

async fn role_assignment(ctx: &Context, reaction: &Reaction) {
    // Check if the reacting user now gets a role
    let key = (reaction.channel_id, reaction.message_id);
    let role: Option<RoleId> = match ctx.data.read().await.get::<ReactionTracker>() {
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
        if let Some(mut member) = get_member(&ctx, reaction.channel_id, reaction.user_id).await {
            let role_name = role
                .to_role_cached(&ctx.cache)
                .await
                .expect("Role not found in cache")
                .name;
            if let Err(why) = member.add_role(&ctx.http, role).await {
                error!("Could not add role to member for reaction: {}", why);
            } else {
                info!(
                    "Assigned role '{}' to member {}",
                    role_name,
                    member.user.read().await.name
                );
            }
        }
    }
}

mod channel;
mod emoji;
mod get_impls;
mod guild;
mod member;
mod role;
mod serde;
mod user;

pub use channel::CachedChannel;
pub use emoji::CachedEmoji;
pub use guild::{CachedGuild, ColdStorageGuild};
pub use member::CachedMember;
pub use role::CachedRole;
pub use user::CachedUser;

use crate::{
    core::{BotStats, Context, ShardState},
    BotResult, Error,
};

use dashmap::DashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use tokio::{
    sync::RwLock,
    time::{timeout, Duration},
};
use twilight::gateway::Event;
use twilight::model::{
    channel::{Channel, GuildChannel, PrivateChannel},
    gateway::{
        payload::RequestGuildMembers,
        presence::{ActivityType, Status},
    },
    id::{ChannelId, EmojiId, GuildId, UserId},
    user::CurrentUser,
};

pub struct Cache {
    pub bot_user: CurrentUser,
    pub guilds: DashMap<GuildId, Arc<CachedGuild>>,
    pub guild_channels: DashMap<ChannelId, Arc<CachedChannel>>,
    pub private_channels: DashMap<ChannelId, Arc<CachedChannel>>,
    pub dm_channels_by_user: DashMap<UserId, Arc<CachedChannel>>,
    pub users: DashMap<UserId, Arc<CachedUser>>,
    pub emoji: DashMap<EmojiId, Arc<CachedEmoji>>,
    pub filling: AtomicBool,

    pub unavailable_guilds: RwLock<Vec<GuildId>>,
    pub expected: RwLock<Vec<GuildId>>,

    pub stats: Arc<BotStats>,
    pub missing_per_shard: DashMap<u64, AtomicU64>,
}

impl Cache {
    pub fn new(bot_user: CurrentUser, stats: Arc<BotStats>) -> Self {
        Cache {
            bot_user,
            guilds: DashMap::new(),
            guild_channels: DashMap::new(),
            private_channels: DashMap::new(),
            dm_channels_by_user: DashMap::new(),
            users: DashMap::new(),
            emoji: DashMap::new(),
            filling: AtomicBool::new(true),
            unavailable_guilds: RwLock::new(vec![]),
            expected: RwLock::new(vec![]),
            stats,
            missing_per_shard: DashMap::new(),
        }
    }

    pub fn reset(&self) {
        self.guilds.clear();
        self.guild_channels.clear();
        self.users.clear();
        self.emoji.clear();
        self.filling.store(true, Ordering::SeqCst);
        self.private_channels.clear();
    }

    pub async fn update(&self, shard_id: u64, event: &Event, ctx: Arc<Context>) -> BotResult<()> {
        match event {
            Event::Ready(_ready) => {} // Potential memory leak
            Event::GuildCreate(e) => {
                trace!("Received guild create event for `{}` ({})", e.name, e.id);
                if let Some(cached_guild) = self.guilds.get(&e.id) {
                    self.nuke_guild_cache(cached_guild.value())
                }
                let guild = CachedGuild::from(e.0.clone());
                for channel in &guild.channels {
                    self.guild_channels
                        .insert(channel.get_id(), channel.value().clone());
                }
                self.stats.channel_count.add(guild.channels.len() as i64);
                for emoji in &guild.emoji {
                    self.emoji.insert(emoji.id, emoji.clone());
                }
                // We dont need this mutable but acquire a write lock regardless to prevent potential deadlocks
                let list_fut = timeout(Duration::from_secs(5), self.unavailable_guilds.write());
                match list_fut.await {
                    Ok(mut list) => {
                        if let Some(index) = list.iter().position(|id| id.0 == guild.id.0) {
                            list.remove(index);
                            info!("Guild `{}` ({}) available again", guild.name, guild.id);
                        }
                    }
                    Err(_) => error!("Timeout while waiting for cache.unavailable_guilds"),
                }
                // Trigger member chunk events
                let data = RequestGuildMembers::new_all(guild.id, None);
                ctx.backend
                    .cluster
                    .command(shard_id, &data)
                    .await
                    .map_err(Error::TwilightCluster)?;
                // Add to cache
                self.guilds.insert(e.id, Arc::new(guild));
                self.stats.guild_counts.partial.inc();
            }
            Event::GuildUpdate(update) => {
                trace!("Receive guild update for `{}` ({})", update.name, update.id);
                match self.get_guild(update.id) {
                    Some(old_guild) => {
                        old_guild.update(&update.0);
                    }
                    None => {
                        warn!(
                            "Got guild update for `{}` ({}) but guild was not found in cache",
                            update.name, update.id
                        );
                    }
                }
            }
            Event::GuildDelete(guild) => {
                if let Some(cached_guild) = self.get_guild(guild.id) {
                    if guild.unavailable {
                        self.guild_unavailable(&cached_guild).await;
                    }
                    self.nuke_guild_cache(&cached_guild)
                }
            }
            Event::MemberChunk(chunk) => {
                trace!(
                    "Received member chunk {}/{} (nonce: {:?}) for guild {}",
                    chunk.chunk_index + 1,
                    chunk.chunk_count,
                    chunk.nonce,
                    chunk.guild_id
                );
                match self.get_guild(chunk.guild_id) {
                    Some(guild) => {
                        let mut count = 0;
                        for (user_id, member) in chunk.members.iter() {
                            if !guild.members.contains_key(user_id) {
                                count += 1;
                                let user = self.get_or_insert_user(&member.user);
                                let member = Arc::new(CachedMember::from_member(member));
                                let count = user.mutual_servers.fetch_add(1, Ordering::SeqCst);
                                trace!(
                                    "User {} received for guild {}, they are now in {} mutuals",
                                    user_id,
                                    guild.id,
                                    count,
                                );
                                guild.members.insert(*user_id, member);
                            }
                        }
                        self.stats.user_counts.total.add(count);
                        if chunk.chunk_count - 1 == chunk.chunk_index && chunk.nonce.is_none() {
                            debug!(
                                "Finished processing chunks for `{}` ({}), {:?} guilds to go...",
                                guild.name,
                                guild.id.0,
                                self.stats.guild_counts.partial.get() - 1
                            );
                            guild.complete.store(true, Ordering::SeqCst);
                            let shard_missing = match self.missing_per_shard.get(&shard_id) {
                                Some(amount) => amount.fetch_sub(1, Ordering::Relaxed),
                                None => {
                                    warn!("shard_id {} not in self.missing_per_shard", shard_id);
                                    0
                                }
                            };
                            if shard_missing == 1 {
                                // this shard is ready
                                info!("All guilds cached for shard {}", shard_id);
                                if chunk.nonce.is_none() && self.shard_cached(shard_id) {
                                    let c = ctx.clone();
                                    tokio::spawn(async move {
                                        let fut = c.set_shard_activity(
                                            shard_id,
                                            Status::Online,
                                            ActivityType::Playing,
                                            "osu!",
                                        );
                                        if let Err(why) = fut.await {
                                            error!(
                                                "Failed to set shard activity for shard {}: {}",
                                                shard_id, why
                                            );
                                        }
                                    });
                                }
                            }
                            self.stats.guild_counts.partial.dec();
                            self.stats.guild_counts.loaded.inc();
                            // if we were at 1 we are now at 0
                            if self.stats.guild_counts.partial.get() == 0
                                && self.filling.load(Ordering::Relaxed)
                                && ctx
                                    .backend
                                    .shard_states
                                    .iter()
                                    .all(|state| state.value() == &ShardState::Ready)
                            {
                                info!("Initial cache filling completed for cluster",);
                                self.filling.store(false, Ordering::SeqCst);
                            }
                        }
                    }
                    None => {
                        error!(
                            "Received member chunks for guild {} before its creation",
                            chunk.guild_id
                        );
                    }
                }
            }

            Event::ChannelCreate(event) => {
                trace!("Received channel create event");
                match &event.0 {
                    Channel::Group(_group) => {}
                    Channel::Guild(guild_channel) => {
                        let guild_id = match guild_channel {
                            GuildChannel::Category(category) => category.guild_id,
                            GuildChannel::Text(text) => text.guild_id,
                            GuildChannel::Voice(voice) => voice.guild_id,
                        };
                        match guild_id {
                            Some(guild_id) => {
                                let channel =
                                    CachedChannel::from_guild_channel(guild_channel, guild_id);
                                match self.get_guild(guild_id) {
                                    Some(guild) => {
                                        let arced = Arc::new(channel);
                                        guild.channels.insert(arced.get_id(), arced.clone());
                                        self.guild_channels.insert(arced.get_id(), arced);
                                        self.stats.channel_count.inc();
                                    }
                                    None => error!(
                                        "Channel create received for `{}` ({}) in guild {} but guild not cached",
                                        channel.get_name(),
                                        channel.get_id(),
                                        guild_id
                                    ),
                                }
                            }
                            None => warn!(
                                "Got channel create event for guild type channel without guild id"
                            ),
                        }
                    }
                    Channel::Private(private_channel) => {
                        self.insert_private_channel(private_channel);
                    }
                };
            }
            Event::ChannelUpdate(channel) => match &channel.0 {
                Channel::Group(_group) => {}
                Channel::Guild(guild_channel) => {
                    let guild_id = match guild_channel {
                        GuildChannel::Category(cateogry) => cateogry.guild_id,
                        GuildChannel::Text(text) => text.guild_id,
                        GuildChannel::Voice(voice) => voice.guild_id,
                    };
                    match guild_id.map(|id| self.get_guild(id)) {
                        Some(Some(guild)) => {
                            let channel =
                                CachedChannel::from_guild_channel(guild_channel, guild.id);
                            let arced = Arc::new(channel);
                            guild.channels.insert(arced.get_id(), arced.clone());
                            self.guild_channels.insert(arced.get_id(), arced);
                        }
                        Some(None) => warn!(
                            "Got channel update for guild {} but guild not cached",
                            guild_id.unwrap()
                        ),
                        None => warn!("Got channel update for guild type channel without guild id"),
                    }
                }
                Channel::Private(private) => {
                    self.insert_private_channel(private);
                }
            },
            Event::ChannelDelete(channel) => {
                match &channel.0 {
                    Channel::Group(_group) => {}
                    Channel::Guild(guild_channel) => {
                        let (guild_id, channel_id) = match guild_channel {
                            GuildChannel::Text(text) => (text.guild_id, text.id),
                            GuildChannel::Voice(voice) => (voice.guild_id, voice.id),
                            GuildChannel::Category(category) => (category.guild_id, category.id),
                        };
                        match guild_id.map(|id| self.get_guild(id)) {
                            Some(Some(guild)) => {
                                self.guild_channels.remove(&channel_id);
                                guild.channels.remove(&channel_id);
                                self.stats.channel_count.dec();
                            }
                            Some(None) => warn!(
                                "Got channel delete event for channel {} \
                                of guild {} but guild not cached",
                                channel_id,
                                guild_id.unwrap()
                            ),
                            None => warn!(
                                "Got channel delete event for channel {} \
                                of some guild but without guild id",
                                channel_id
                            ),
                        }
                    }
                    // Do these even ever get deleted?
                    Channel::Private(channel) => {
                        self.private_channels.remove(&channel.id);
                        if channel.recipients.len() == 1 {
                            self.dm_channels_by_user.remove(&channel.recipients[0].id);
                        }
                    }
                }
            }

            Event::MemberAdd(event) => {
                trace!("{} joined {}", event.user.id, event.guild_id);
                match self.get_guild(event.guild_id) {
                    Some(guild) => {
                        let member = CachedMember::from_member(&event.0);
                        match self.get_user(event.user.id) {
                            Some(user) => {
                                user.mutual_servers.fetch_add(1, Ordering::SeqCst);
                            }
                            None => {
                                self.get_or_insert_user(&event.user);
                            }
                        }
                        guild.members.insert(event.user.id, Arc::new(member));
                        guild.member_count.fetch_add(1, Ordering::Relaxed);
                        self.stats.user_counts.total.inc();
                    }
                    None => warn!(
                        "Received member add event for guild {} before guild create",
                        event.guild_id
                    ),
                }
            }
            Event::MemberUpdate(event) => {
                trace!("Member {} updated in {}", event.user.id, event.guild_id);
                match ctx.cache.get_guild(event.guild_id) {
                    Some(guild) => {
                        match ctx.cache.get_user(event.user.id) {
                            Some(user) => {
                                if !user.is_same_as(&event.user) {
                                    // Just update the global cache if it's different
                                    // we will receive an event for all mutual servers if the inner user changed
                                    let new_user = Arc::new(CachedUser::from_user(&event.user));
                                    new_user.mutual_servers.store(
                                        user.mutual_servers.load(Ordering::SeqCst),
                                        Ordering::SeqCst,
                                    );
                                    ctx.cache.users.insert(event.user.id, new_user);
                                }
                            }
                            None => {
                                if guild.complete.load(Ordering::SeqCst) {
                                    warn!(
                                        "Received member update with uncached inner user: {}",
                                        event.user.id
                                    );
                                    ctx.cache.get_or_insert_user(&event.user);
                                }
                            }
                        }
                        let member = guild
                            .members
                            .get(&event.user.id)
                            .map(|guard| guard.value().clone());
                        match member {
                            Some(member) => {
                                let updated = member.update(&*event);
                                guild.members.insert(member.user_id, Arc::new(updated));
                            }
                            None => {
                                if guild.complete.load(Ordering::SeqCst) {
                                    warn!(
                                        "Received member update for unknown member {} in guild {}",
                                        event.user.id, guild.id
                                    );
                                    let user = event.user.id;
                                    let guild_id = guild.id;
                                    tokio::spawn(async move {
                                        let data = RequestGuildMembers::new_single_user_with_nonce(
                                            guild_id,
                                            user,
                                            None,
                                            Some(String::from("missing_user")),
                                        );
                                        let _ = ctx.backend.cluster.command(shard_id, &data).await;
                                    });
                                }
                            }
                        }
                    }
                    None => {
                        warn!(
                            "Received member update for uncached guild {}",
                            event.guild_id
                        );
                    }
                };
            }
            Event::MemberRemove(event) => {
                trace!("{} left {}", event.user.id, event.guild_id);
                match self.get_guild(event.guild_id) {
                    Some(guild) => match guild.members.remove(&event.user.id) {
                        Some((_, member)) => match member.user(self) {
                            Some(user) => {
                                if user.mutual_servers.fetch_sub(1, Ordering::SeqCst) == 1 {
                                    self.users.remove(&member.user_id);
                                    self.stats.user_counts.unique.dec();
                                }
                                self.stats.user_counts.total.dec();
                            }
                            None => debug!(
                                "User of member {} not in cache for MemberRemove",
                                member.user_id
                            ),
                        },
                        None => {
                            if guild.complete.load(Ordering::SeqCst) {
                                warn!("Received member remove event for member that is not in that guild");
                            }
                        }
                    },
                    None => warn!(
                        "Received member remove event for guild {} but guild not cached",
                        event.guild_id
                    ),
                }
            }

            Event::RoleCreate(event) => match self.get_guild(event.guild_id) {
                Some(guild) => {
                    guild
                        .roles
                        .insert(event.role.id, Arc::new(CachedRole::from_role(&event.role)));
                }
                None => warn!(
                    "Received role create event for guild {} but guild not cached",
                    event.guild_id
                ),
            },
            Event::RoleUpdate(event) => match self.get_guild(event.guild_id) {
                Some(guild) => {
                    guild
                        .roles
                        .insert(event.role.id, Arc::new(CachedRole::from_role(&event.role)));
                }
                None => warn!(
                    "Received role update event for guild {} but guild not cached",
                    event.guild_id
                ),
            },
            Event::RoleDelete(event) => match self.get_guild(event.guild_id) {
                Some(guild) => {
                    guild.roles.remove(&event.role_id);
                }
                None => warn!(
                    "Received role delete event for guild {} but guild not cached",
                    event.guild_id
                ),
            },
            _ => {}
        }
        Ok(())
    }

    // ###################
    // ## Cache updates ##
    // ###################

    fn nuke_guild_cache(&self, guild: &CachedGuild) {
        for channel in &guild.channels {
            self.guild_channels.remove(channel.key());
        }
        self.stats.channel_count.sub(guild.channels.len() as i64);
        for member in &guild.members {
            match member.user(self) {
                Some(user) => {
                    if user.mutual_servers.fetch_sub(1, Ordering::SeqCst) == 1 {
                        self.users.remove(&member.user_id);
                        self.stats.user_counts.unique.dec();
                    }
                }
                None => debug!(
                    "User of member {} not in cache for nuke_guild_cache",
                    member.user_id
                ),
            }
        }
        self.stats.user_counts.total.sub(guild.members.len() as i64);
        for emoji in &guild.emoji {
            self.emoji.remove(&emoji.id);
        }
    }

    async fn guild_unavailable(&self, guild: &CachedGuild) {
        warn!(
            "Guild `{}` ({}) became unavailable due to outage",
            guild.name, guild.id
        );
        self.stats.guild_counts.outage.inc();
        let list_fut = timeout(Duration::from_secs(5), self.unavailable_guilds.write());
        match list_fut.await {
            Ok(mut list) => {
                list.push(guild.id);
            }
            Err(_) => error!("Timeout while waiting for cache.unavailable_guilds"),
        }
    }

    pub fn insert_private_channel(&self, private_channel: &PrivateChannel) -> Arc<CachedChannel> {
        let channel = CachedChannel::from_private(private_channel, self);
        let arced = Arc::new(channel);
        if let CachedChannel::DM { receiver, .. } = arced.as_ref() {
            self.dm_channels_by_user.insert(receiver.id, arced.clone());
        }
        self.private_channels.insert(arced.get_id(), arced.clone());
        arced
    }

    pub fn shard_cached(&self, shard_id: u64) -> bool {
        match self.missing_per_shard.get(&shard_id) {
            Some(atomic) => atomic.value().load(Ordering::Relaxed) == 0,
            None => true, // we cold resumed so have everything
        }
    }
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

fn is_true(t: &bool) -> bool {
    !t
}

fn get_true() -> bool {
    true
}

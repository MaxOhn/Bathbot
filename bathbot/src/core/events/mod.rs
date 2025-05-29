use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
};

use bathbot_cache::model::CachedArchive;
use bathbot_model::twilight::{channel::ArchivedCachedChannel, guild::ArchivedCachedGuild};
use bathbot_util::{Authored, BucketName, constants::MISS_ANALYZER_ID};
use eyre::Result;
use tokio::{
    sync::{Mutex, broadcast::Receiver},
    task::JoinSet,
};
use twilight_gateway::{Event, EventTypeFlags, Shard, StreamExt as _};
use twilight_model::user::User;

use self::{interaction::handle_interaction, message::handle_message};
use super::{BotMetrics, Context};

mod interaction;
mod message;

#[derive(Debug)]
enum ProcessResult {
    Success,
    NoDM,
    NoSendPermission,
    Ratelimited(
        // false positive; used when logging
        #[allow(unused)] BucketName,
    ),
    NoOwner,
    NoAuthority,
}

pub enum EventKind {
    Autocomplete,
    Component,
    Modal,
    PrefixCommand,
    InteractionCommand,
}

impl EventKind {
    pub async fn log<A>(self, orig: &A, name: &str)
    where
        A: Authored + Send + Sync,
    {
        fn log(kind: EventKind, location: &EventLocation, user: Result<&User>, name: &str) {
            let username = user.map_or("<unknown user>", |u| u.name.as_str());

            info!("[{location}] {username} {kind} `{name}`");
        }

        let location = EventLocation::new(orig).await;
        log(self, &location, orig.user(), name);
    }
}

impl Display for EventKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Autocomplete => f.write_str("autocompleted"),
            Self::Component => f.write_str("used component"),
            Self::Modal => f.write_str("used modal"),
            Self::PrefixCommand => f.write_str("used prefix command"),
            Self::InteractionCommand => f.write_str("used interaction command"),
        }
    }
}

enum EventLocation {
    Private,
    UncachedGuild,
    UncachedChannel {
        guild: CachedArchive<ArchivedCachedGuild>,
    },
    Cached {
        guild: CachedArchive<ArchivedCachedGuild>,
        channel: CachedArchive<ArchivedCachedChannel>,
    },
}

impl EventLocation {
    async fn new<A>(orig: &A) -> Self
    where
        A: Authored + Send + Sync,
    {
        let Some(guild_id) = orig.guild_id() else {
            return Self::Private;
        };

        let cache = Context::cache();

        let Ok(Some(guild)) = cache.guild(guild_id).await else {
            return Self::UncachedGuild;
        };

        let Ok(Some(channel)) = cache.channel(Some(guild_id), orig.channel_id()).await else {
            return Self::UncachedChannel { guild };
        };

        Self::Cached { guild, channel }
    }
}

impl Display for EventLocation {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            EventLocation::Private => f.write_str("Private"),
            EventLocation::UncachedGuild => f.write_str("<uncached guild>"),
            EventLocation::UncachedChannel { guild } => {
                write!(f, "{}:<uncached channel>", guild.id)
            }
            EventLocation::Cached { guild, channel } => write!(f, "{}:{}", guild.id, channel.id),
        }
    }
}

const EVENT_FLAGS: EventTypeFlags = EventTypeFlags::CHANNEL_CREATE
    .union(EventTypeFlags::CHANNEL_DELETE)
    .union(EventTypeFlags::CHANNEL_UPDATE)
    .union(EventTypeFlags::GUILD_CREATE)
    .union(EventTypeFlags::GUILD_DELETE)
    .union(EventTypeFlags::GUILD_UPDATE)
    .union(EventTypeFlags::INTERACTION_CREATE)
    .union(EventTypeFlags::MEMBER_ADD)
    .union(EventTypeFlags::MEMBER_REMOVE)
    .union(EventTypeFlags::MEMBER_UPDATE)
    .union(EventTypeFlags::MEMBER_CHUNK)
    .union(EventTypeFlags::MESSAGE_CREATE)
    .union(EventTypeFlags::MESSAGE_DELETE)
    .union(EventTypeFlags::MESSAGE_DELETE_BULK)
    .union(EventTypeFlags::READY)
    .union(EventTypeFlags::ROLE_CREATE)
    .union(EventTypeFlags::ROLE_DELETE)
    .union(EventTypeFlags::ROLE_UPDATE)
    .union(EventTypeFlags::THREAD_CREATE)
    .union(EventTypeFlags::THREAD_DELETE)
    .union(EventTypeFlags::THREAD_UPDATE)
    .union(EventTypeFlags::UNAVAILABLE_GUILD)
    .union(EventTypeFlags::USER_UPDATE);

pub async fn event_loop(
    runners: &mut JoinSet<()>,
    shards: &mut Vec<Arc<Mutex<Shard>>>,
    mut reshard_rx: Receiver<()>,
) {
    loop {
        for shard in shards.iter() {
            runners.spawn(runner(Arc::clone(shard), reshard_rx.resubscribe()));
        }

        while runners.join_next().await.is_some() {}

        if let Err(err) = Context::reshard(shards).await {
            return error!("{err:?}");
        }

        while !reshard_rx.is_empty() {
            let _: Result<_, _> = reshard_rx.recv().await;
        }
    }
}

async fn runner(shard: Arc<Mutex<Shard>>, mut reshard_rx: Receiver<()>) {
    let standby = Context::standby();
    let cache = Context::cache();
    let mut shard = shard.lock().await;
    let shard_id = shard.id().number();

    loop {
        tokio::select!(
             res = shard.next_event(EVENT_FLAGS)  => match res {
                Some(Ok(event)) => {
                    standby.process(&event);
                    let change = cache.update(&event).await;
                    BotMetrics::event(&event, change);
                    tokio::spawn(handle_event(event, shard_id));
                }
                Some(Err(err)) => error!(?err, "Event error"),
                None => return,
            },
            _ = reshard_rx.recv() => return,
        );
    }
}

async fn handle_event(event: Event, shard_id: u32) {
    async fn inner(event: Event, shard_id: u32) -> Result<()> {
        match event {
            Event::GatewayClose(Some(frame)) => {
                warn!(
                    shard_id,
                    reason = frame.reason.as_ref(),
                    code = frame.code,
                    "Received closing frame"
                )
            }
            Event::GatewayClose(None) => {
                warn!(shard_id, "Received closing frame")
            }
            Event::GatewayInvalidateSession(true) => {
                warn!(
                    shard_id,
                    "Gateway has invalidated session but its reconnectable"
                )
            }
            Event::GatewayInvalidateSession(false) => {
                warn!(shard_id, "Gateway has invalidated session")
            }
            Event::GatewayReconnect => {
                info!(shard_id, "Gateway requested shard to reconnect")
            }
            Event::GuildCreate(e) => {
                let guild_id = e.id();
                let ctx = Context::get();

                ctx.guild_shards().pin().insert(guild_id, shard_id);
                ctx.member_requests
                    .pending_guilds
                    .lock()
                    .unwrap()
                    .insert(guild_id);

                if let Err(err) = ctx.member_requests.tx.send((guild_id, shard_id)) {
                    warn!(?err, "Failed to forward member request");
                }
            }
            Event::InteractionCreate(e) => handle_interaction(e.0).await,
            Event::MemberAdd(e) if e.member.user.id == MISS_ANALYZER_ID => {
                Context::miss_analyzer_guilds()
                    .write()
                    .unwrap()
                    .insert(e.guild_id);
            }
            Event::MemberChunk(e) => {
                if e.members
                    .iter()
                    .any(|member| member.user.id == MISS_ANALYZER_ID)
                {
                    Context::miss_analyzer_guilds()
                        .write()
                        .unwrap()
                        .insert(e.guild_id);
                }
            }
            Event::MemberRemove(e) if e.user.id == MISS_ANALYZER_ID => {
                Context::miss_analyzer_guilds()
                    .write()
                    .unwrap()
                    .remove(&e.guild_id);
            }
            Event::MessageCreate(msg) => handle_message(msg.0).await,
            Event::MessageDelete(e) => {
                Context::get().active_msgs.remove(e.id).await;
            }
            Event::MessageDeleteBulk(msgs) => {
                for id in msgs.ids.into_iter() {
                    Context::get().active_msgs.remove(id).await;
                }
            }
            Event::Ready(_) => info!(shard_id, "Shard is ready"),
            Event::Resumed => info!(shard_id, "Shard is resumed"),
            _ => {}
        }

        Ok(())
    }

    if let Err(err) = inner(event, shard_id).await {
        error!(?err, "Failed to handle event");
    }
}

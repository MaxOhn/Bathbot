use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
};

use bathbot_cache::model::CachedArchive;
use bathbot_model::twilight_model::{channel::Channel, guild::Guild};
use bathbot_util::constants::MISS_ANALYZER_ID;
use eyre::Result;
use futures::StreamExt;
use twilight_gateway::{error::ReceiveMessageErrorType, stream::ShardEventStream, Event, Shard};
use twilight_model::{gateway::CloseCode, user::User};

use self::{interaction::handle_interaction, message::handle_message};
use super::{buckets::BucketName, Context};
use crate::util::Authored;

mod interaction;
mod message;

#[derive(Debug)]
enum ProcessResult {
    Success,
    NoDM,
    NoSendPermission,
    Ratelimited(BucketName),
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
    pub async fn log<A>(self, ctx: &Context, orig: &A, name: &str)
    where
        A: Authored + Send + Sync,
    {
        fn log(kind: EventKind, location: &EventLocation, user: Result<&User>, name: &str) {
            let username = user.map_or("<unknown user>", |u| u.name.as_str());

            info!("[{location}] {username} {kind} `{name}`");
        }

        let location = EventLocation::new(ctx, orig).await;
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
        guild: CachedArchive<Guild>,
    },
    Cached {
        guild: CachedArchive<Guild>,
        channel: CachedArchive<Channel>,
    },
}

impl EventLocation {
    async fn new<A>(ctx: &Context, orig: &A) -> Self
    where
        A: Authored + Send + Sync,
    {
        let Some(guild_id) = orig.guild_id() else {
            return Self::Private
        };

        let Ok(Some(guild)) = ctx.cache.guild(guild_id).await else {
            return Self::UncachedGuild
        };

        let Ok(Some(channel)) = ctx.cache.channel(Some(guild_id), orig.channel_id()).await else {
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
                write!(f, "{}:<uncached channel>", guild.name)
            }
            EventLocation::Cached { guild, channel } => match channel.name.as_ref() {
                Some(channel_name) => write!(f, "{}:{channel_name}", guild.name),
                None => write!(f, "{}:<no channel name>", guild.name),
            },
        }
    }
}

pub async fn event_loop(ctx: Arc<Context>, shards: &mut Vec<Shard>) {
    // restarts event loop in case the bot was instructed to reshard
    'reshard_loop: loop {
        let mut stream = ShardEventStream::new(shards.iter_mut());

        // actual event loop
        'event_loop: loop {
            let err = match stream.next().await {
                Some((shard, Ok(event))) => {
                    ctx.standby.process(&event);
                    let change = ctx.cache.update(&event).await;
                    ctx.stats.process(&event, change);
                    let ctx = Arc::clone(&ctx);
                    let shard_id = shard.id().number();

                    tokio::spawn(async move {
                        if let Err(err) = handle_event(ctx, event, shard_id).await {
                            error!(?err, "Failed to handle event");
                        }
                    });

                    continue 'event_loop;
                }
                Some((_, Err(err))) => err,
                None => return,
            };

            // cannot be handled inside the previous `match` due to NLL
            // https://github.com/rust-lang/rust/issues/43234
            let is_fatal = err.is_fatal();

            let must_reshard = matches!(
                err.kind(),
                ReceiveMessageErrorType::FatallyClosed {
                    close_code: CloseCode::ShardingRequired
                }
            );

            error!(%err, "Event error");

            if must_reshard {
                drop(stream);

                if let Err(err) = ctx.reshard(shards).await {
                    return error!("{err:?}");
                }

                continue 'reshard_loop;
            } else if is_fatal {
                return;
            }
        }
    }
}

async fn handle_event(ctx: Arc<Context>, event: Event, shard_id: u64) -> Result<()> {
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
            ctx.guild_shards().pin().insert(e.id, shard_id);
            ctx.member_requests.todo_guilds.lock().insert(e.id);

            if let Err(err) = ctx.member_requests.tx.send((e.id, shard_id)) {
                warn!(?err, "Failed to forward member request");
            }
        }
        Event::InteractionCreate(e) => handle_interaction(ctx, e.0).await,
        Event::MemberAdd(e) if e.member.user.id == MISS_ANALYZER_ID => {
            ctx.miss_analyzer_guilds().pin().insert(e.guild_id);
        }
        Event::MemberChunk(e) => {
            if e.members
                .iter()
                .any(|member| member.user.id == MISS_ANALYZER_ID)
            {
                ctx.miss_analyzer_guilds().pin().insert(e.guild_id);
            }
        }
        Event::MemberRemove(e) if e.user.id == MISS_ANALYZER_ID => {
            ctx.miss_analyzer_guilds().pin().remove(&e.guild_id);
        }
        Event::MessageCreate(msg) => handle_message(ctx, msg.0).await,
        Event::MessageDelete(e) => {
            ctx.remove_msg(e.id);
        }
        Event::MessageDeleteBulk(msgs) => {
            for id in msgs.ids.into_iter() {
                ctx.remove_msg(id);
            }
        }
        Event::Ready(_) => info!(shard_id, "Shard is ready"),
        Event::Resumed => info!(shard_id, "Shard is resumed"),
        _ => {}
    }

    Ok(())
}

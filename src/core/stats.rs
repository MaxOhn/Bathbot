use crate::{core::Context, unwind_error};

use chrono::{DateTime, Utc};
use log::info;
use prometheus::{IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry};
use std::{
    collections::HashMap,
    sync::{atomic::Ordering::Relaxed, Arc},
};
use twilight_cache_inmemory::Metrics;
use twilight_model::{channel::Message, gateway::event::Event};

pub struct EventStats {
    pub gateway_reconnect: IntCounter,
    pub channel_create: IntCounter,
    pub channel_delete: IntCounter,
    pub guild_create: IntCounter,
    pub guild_delete: IntCounter,
    pub guild_update: IntCounter,
    pub member_add: IntCounter,
    pub member_remove: IntCounter,
    pub member_update: IntCounter,
    pub member_chunk: IntCounter,
    pub message_create: IntCounter,
    pub message_delete: IntCounter,
    pub message_delete_bulk: IntCounter,
    pub message_update: IntCounter,
    pub reaction_add: IntCounter,
    pub reaction_remove: IntCounter,
    pub reaction_remove_all: IntCounter,
    pub reaction_remove_emoji: IntCounter,
    pub unavailable_guild: IntCounter,
    pub user_update: IntCounter,
}

pub struct MessageCounters {
    pub user_messages: IntCounter,
    pub other_bot_messages: IntCounter,
    pub own_messages: IntCounter,
}

pub struct UserCounters {
    pub unique: IntGauge,
    pub total: IntGauge,
}

pub struct GuildCounters {
    pub total: IntGauge,
    pub unavailable: IntGauge,
}

pub struct BotStats {
    pub registry: Registry,
    pub start_time: DateTime<Utc>,
    pub event_counts: EventStats,
    pub message_counts: MessageCounters,
    pub user_counts: UserCounters,
    pub channel_count: IntGauge,
    pub guild_counts: GuildCounters,
    pub command_counts: IntCounterVec,
    pub osu_metrics: IntCounterVec,
    pub cache_metrics: Arc<Metrics>,
}

impl BotStats {
    #[rustfmt::skip]
    pub fn new(osu_metrics: IntCounterVec, cache_metrics: Arc<Metrics>) -> Self {
        let event_counter = IntCounterVec::new(Opts::new("gateway_events", "Events received from the gateway"), &["events"]).unwrap();
        let message_counter =IntCounterVec::new(Opts::new("messages", "Recieved messages"), &["sender_type"]).unwrap();
        let user_counter =IntGaugeVec::new(Opts::new("user_counts", "User counts"), &["type"]).unwrap();
        let channel_count = IntGauge::with_opts(Opts::new("channels", "Channel count")).unwrap();
        let guild_counter =IntGaugeVec::new(Opts::new("guild_counts", "State of the guilds"), &["state"]).unwrap();
        let command_counts =IntCounterVec::new(Opts::new("commands", "Executed commands"), &["name"]).unwrap();

        let mut static_labels = HashMap::new();
        static_labels.insert(String::from("cluster"), 0.to_string());
        let registry =Registry::new_custom(Some(String::from("bathbot")), Some(static_labels)).unwrap();
        registry.register(Box::new(event_counter.clone())).unwrap();
        registry.register(Box::new(message_counter.clone())).unwrap();
        registry.register(Box::new(user_counter.clone())).unwrap();
        registry.register(Box::new(channel_count.clone())).unwrap();
        registry.register(Box::new(guild_counter.clone())).unwrap();
        registry.register(Box::new(command_counts.clone())).unwrap();
        registry.register(Box::new(osu_metrics.clone())).unwrap();
        let stats = Self {
            registry,
            start_time: Utc::now(),
            event_counts: EventStats {
                channel_create: event_counter.get_metric_with_label_values(&["ChannelCreate"]).unwrap(),
                channel_delete: event_counter.get_metric_with_label_values(&["ChannelDelete"]).unwrap(),
                gateway_reconnect: event_counter.get_metric_with_label_values(&["GatewayReconnect"]).unwrap(),
                guild_create: event_counter.get_metric_with_label_values(&["GuildCreate"]).unwrap(),
                guild_delete: event_counter.get_metric_with_label_values(&["GuildDelete"]).unwrap(),
                guild_update: event_counter.get_metric_with_label_values(&["GuildUpdate"]).unwrap(),
                member_add: event_counter.get_metric_with_label_values(&["MemberAdd"]).unwrap(),
                member_remove: event_counter.get_metric_with_label_values(&["MemberRemove"]).unwrap(),
                member_update: event_counter.get_metric_with_label_values(&["MemberUpdate"]).unwrap(),
                member_chunk: event_counter.get_metric_with_label_values(&["MemberChunk"]).unwrap(),
                message_create: event_counter.get_metric_with_label_values(&["MessageCreate"]).unwrap(),
                message_delete: event_counter.get_metric_with_label_values(&["MessageDelete"]).unwrap(),
                message_delete_bulk: event_counter.get_metric_with_label_values(&["MessageDeleteBulk"]).unwrap(),
                message_update: event_counter.get_metric_with_label_values(&["MessageUpdate"]).unwrap(),
                reaction_add: event_counter.get_metric_with_label_values(&["ReactionAdd"]).unwrap(),
                reaction_remove: event_counter.get_metric_with_label_values(&["ReactionRemove"]).unwrap(),
                reaction_remove_all: event_counter.get_metric_with_label_values(&["ReactionRemoveAll"]).unwrap(),
                reaction_remove_emoji: event_counter.get_metric_with_label_values(&["ReactionRemoveEmoji"]).unwrap(),
                unavailable_guild: event_counter.get_metric_with_label_values(&["UnavailableGuild"]).unwrap(),
                user_update: event_counter.get_metric_with_label_values(&["UserUpdate"]).unwrap(),
            },
            message_counts: MessageCounters {
                user_messages: message_counter.get_metric_with_label_values(&["User"]).unwrap(),
                other_bot_messages: message_counter.get_metric_with_label_values(&["Bot"]).unwrap(),
                own_messages: message_counter.get_metric_with_label_values(&["Own"]).unwrap(),
            },
            user_counts: UserCounters {
                unique: user_counter.get_metric_with_label_values(&["Unique"]).unwrap(),
                total: user_counter.get_metric_with_label_values(&["Total"]).unwrap(),
            },
            guild_counts: GuildCounters {
                total: guild_counter.get_metric_with_label_values(&["Total"]).unwrap(),
                unavailable: guild_counter.get_metric_with_label_values(&["Unavailable"]).unwrap(),
            },
            channel_count,
            command_counts,
            osu_metrics,
            cache_metrics
        };
        stats
            .guild_counts
            .total
            .set(stats.cache_metrics.guilds.load(Relaxed) as i64);
        stats
            .guild_counts
            .unavailable
            .set(stats.cache_metrics.unavailable_guilds.load(Relaxed) as i64);
        stats
            .user_counts
            .total
            .set(stats.cache_metrics.members.load(Relaxed) as i64);
        stats
            .user_counts
            .unique
            .set(stats.cache_metrics.users.load(Relaxed) as i64);
        stats
    }

    pub fn new_message(&self, ctx: &Context, msg: &Message) {
        if msg.author.bot {
            if ctx.is_own(msg) {
                self.message_counts.own_messages.inc()
            } else {
                self.message_counts.other_bot_messages.inc()
            }
        } else {
            self.message_counts.user_messages.inc()
        }
    }

    pub fn inc_command(&self, cmd: impl AsRef<str>) {
        let c = cmd.as_ref();
        match self.command_counts.get_metric_with_label_values(&[c]) {
            Ok(counter) => counter.inc(),
            Err(why) => unwind_error!(warn, why, "Error while incrementing `{}`'s counter: {}", c),
        }
    }
}

impl Context {
    pub fn update_stats(&self, shard_id: u64, event: &Event) {
        match event {
            Event::ChannelCreate(_) => self.stats.event_counts.channel_create.inc(),
            Event::ChannelDelete(_) => self.stats.event_counts.channel_delete.inc(),
            Event::GatewayReconnect => self.stats.event_counts.gateway_reconnect.inc(),
            Event::GuildCreate(_) => {
                self.stats
                    .guild_counts
                    .total
                    .set(self.stats.cache_metrics.guilds.load(Relaxed) as i64);
                self.stats
                    .guild_counts
                    .unavailable
                    .set(self.stats.cache_metrics.unavailable_guilds.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .total
                    .set(self.stats.cache_metrics.members.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .unique
                    .set(self.stats.cache_metrics.users.load(Relaxed) as i64);
                self.stats.event_counts.guild_create.inc()
            }
            Event::GuildDelete(_) => {
                self.stats
                    .guild_counts
                    .total
                    .set(self.stats.cache_metrics.guilds.load(Relaxed) as i64);
                self.stats
                    .guild_counts
                    .unavailable
                    .set(self.stats.cache_metrics.unavailable_guilds.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .total
                    .set(self.stats.cache_metrics.members.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .unique
                    .set(self.stats.cache_metrics.users.load(Relaxed) as i64);
                self.stats.event_counts.guild_delete.inc()
            }
            Event::GuildUpdate(_) => self.stats.event_counts.guild_update.inc(),
            Event::MemberAdd(_) => {
                self.stats
                    .user_counts
                    .total
                    .set(self.stats.cache_metrics.members.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .unique
                    .set(self.stats.cache_metrics.users.load(Relaxed) as i64);
                self.stats.event_counts.member_add.inc()
            }
            Event::MemberRemove(_) => {
                self.stats
                    .user_counts
                    .total
                    .set(self.stats.cache_metrics.members.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .unique
                    .set(self.stats.cache_metrics.users.load(Relaxed) as i64);
                self.stats.event_counts.member_remove.inc()
            }
            Event::MemberUpdate(_) => self.stats.event_counts.member_update.inc(),
            Event::MemberChunk(_) => {
                self.stats
                    .user_counts
                    .total
                    .set(self.stats.cache_metrics.members.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .unique
                    .set(self.stats.cache_metrics.users.load(Relaxed) as i64);
                self.stats.event_counts.member_chunk.inc()
            }
            Event::MessageCreate(_) => self.stats.event_counts.message_create.inc(),
            Event::MessageDelete(_) => self.stats.event_counts.message_delete.inc(),
            Event::MessageDeleteBulk(_) => self.stats.event_counts.message_delete_bulk.inc(),
            Event::MessageUpdate(_) => self.stats.event_counts.message_update.inc(),
            Event::ReactionAdd(_) => self.stats.event_counts.reaction_add.inc(),
            Event::ReactionRemove(_) => self.stats.event_counts.reaction_remove.inc(),
            Event::ReactionRemoveAll(_) => self.stats.event_counts.reaction_remove_all.inc(),
            Event::ReactionRemoveEmoji(_) => self.stats.event_counts.reaction_remove_emoji.inc(),
            Event::UnavailableGuild(_) => {
                self.stats
                    .guild_counts
                    .total
                    .set(self.stats.cache_metrics.guilds.load(Relaxed) as i64);
                self.stats
                    .guild_counts
                    .unavailable
                    .set(self.stats.cache_metrics.unavailable_guilds.load(Relaxed) as i64);
                self.stats.event_counts.unavailable_guild.inc()
            }
            Event::UserUpdate(_) => self.stats.event_counts.user_update.inc(),

            Event::ShardConnecting(_) => info!("Shard {} is now Connecting", shard_id),
            Event::ShardIdentifying(_) => info!("Shard {} is now Identifying", shard_id),
            Event::ShardConnected(_) => info!("Shard {} is now Connected", shard_id),
            Event::Ready(_) => {
                self.stats
                    .guild_counts
                    .total
                    .set(self.stats.cache_metrics.guilds.load(Relaxed) as i64);
                self.stats
                    .guild_counts
                    .unavailable
                    .set(self.stats.cache_metrics.unavailable_guilds.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .total
                    .set(self.stats.cache_metrics.members.load(Relaxed) as i64);
                self.stats
                    .user_counts
                    .unique
                    .set(self.stats.cache_metrics.users.load(Relaxed) as i64);
                info!("Shard {} is now Ready", shard_id)
            }
            Event::Resumed => info!("Shard {} is now Resumed", shard_id),
            Event::ShardResuming(_) => info!("Shard {} is now Resuming", shard_id),
            Event::ShardReconnecting(_) => info!("Shard {} is now Reconnecting", shard_id),
            Event::ShardDisconnected(_) => info!("Shard {} is now Disconnected", shard_id),
            _ => {}
        }
    }
}

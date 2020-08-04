use crate::core::{Context, ShardState};

use chrono::{DateTime, Utc};
use log::info;
use prometheus::{IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry};
use std::collections::HashMap;
use twilight::model::{channel::Message, gateway::event::Event};

pub struct EventStats {
    pub channel_create: IntCounter,
    pub channel_delete: IntCounter,
    pub gateway_reconnect: IntCounter,
    pub channel_pins_update: IntCounter,
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
    pub partial: IntGauge,
    pub loaded: IntGauge,
    pub outage: IntGauge,
}

pub struct ShardStats {
    pub pending: IntGauge,
    pub connecting: IntGauge,
    pub identifying: IntGauge,
    pub connected: IntGauge,
    pub ready: IntGauge,
    pub resuming: IntGauge,
    pub reconnecting: IntGauge,
    pub disconnected: IntGauge,
}

pub struct BotStats {
    pub registry: Registry,
    pub start_time: DateTime<Utc>,
    pub event_counts: EventStats,
    pub message_counts: MessageCounters,
    pub user_counts: UserCounters,
    pub shard_counts: ShardStats,
    pub channel_count: IntGauge,
    pub guild_counts: GuildCounters,
    pub command_counts: IntCounterVec,
}

impl BotStats {
    #[rustfmt::skip]
    pub fn new() -> Self {
        let event_counter = IntCounterVec::new(Opts::new("gateway_events", "Events received from the gateway"), &["events"]).unwrap();
        let message_counter =IntCounterVec::new(Opts::new("messages", "Recieved messages"), &["sender_type"]).unwrap();
        let user_counter =IntGaugeVec::new(Opts::new("user_counts", "User counts"), &["type"]).unwrap();
        let shard_counter = IntGaugeVec::new(Opts::new("shard_counts", "State counts for our shards"),&["state"],).unwrap();
        let channel_count = IntGauge::with_opts(Opts::new("channels", "Channel count")).unwrap();
        let guild_counter =IntGaugeVec::new(Opts::new("guild_counts", "State of the guilds"), &["state"]).unwrap();
        let command_counts =IntCounterVec::new(Opts::new("commands", "Executed commands"), &["name"]).unwrap();

        let mut static_labels = HashMap::new();
        static_labels.insert(String::from("cluster"), 0.to_string());
        let registry =Registry::new_custom(Some(String::from("bathbot")), Some(static_labels)).unwrap();
        registry.register(Box::new(event_counter.clone())).unwrap();
        registry.register(Box::new(message_counter.clone())).unwrap();
        registry.register(Box::new(user_counter.clone())).unwrap();
        registry.register(Box::new(shard_counter.clone())).unwrap();
        registry.register(Box::new(channel_count.clone())).unwrap();
        registry.register(Box::new(guild_counter.clone())).unwrap();
        registry.register(Box::new(command_counts.clone())).unwrap();
        Self {
            registry,
            start_time: Utc::now(),
            event_counts: EventStats {
                channel_create: event_counter.get_metric_with_label_values(&["ChannelCreate"]).unwrap(),
                channel_delete: event_counter.get_metric_with_label_values(&["ChannelDelete"]).unwrap(),
                gateway_reconnect: event_counter.get_metric_with_label_values(&["GatewayReconnect"]).unwrap(),
                channel_pins_update: event_counter.get_metric_with_label_values(&["ChannelPinsUpdate"]).unwrap(),
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
                user_messages: message_counter.get_metric_with_label_values(&["user"]).unwrap(),
                other_bot_messages: message_counter.get_metric_with_label_values(&["bot"]).unwrap(),
                own_messages: message_counter.get_metric_with_label_values(&["own"]).unwrap(),
            },
            user_counts: UserCounters {
                unique: user_counter.get_metric_with_label_values(&["unique"]).unwrap(),
                total: user_counter.get_metric_with_label_values(&["total"]).unwrap(),
            },
            guild_counts: GuildCounters {
                partial: guild_counter.get_metric_with_label_values(&["partial"]).unwrap(),
                loaded: guild_counter.get_metric_with_label_values(&["loaded"]).unwrap(),
                outage: guild_counter.get_metric_with_label_values(&["outage"]).unwrap(),
            },
            channel_count,
            shard_counts: ShardStats {
                pending: shard_counter.get_metric_with_label_values(&["pending"]).unwrap(),
                connecting: shard_counter.get_metric_with_label_values(&["connecting"]).unwrap(),
                identifying: shard_counter.get_metric_with_label_values(&["identifying"]).unwrap(),
                connected: shard_counter.get_metric_with_label_values(&["connected"]).unwrap(),
                ready: shard_counter.get_metric_with_label_values(&["ready"]).unwrap(),
                resuming: shard_counter.get_metric_with_label_values(&["resuming"]).unwrap(),
                reconnecting: shard_counter.get_metric_with_label_values(&["reconnecting"]).unwrap(),
                disconnected: shard_counter.get_metric_with_label_values(&["disconnected"]).unwrap(),
            },
            command_counts,
        }
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
            Err(why) => warn!("Error while incrementing `{}`'s counter: {}", c, why),
        }
    }
}

impl Context {
    pub fn update_stats(&self, shard_id: u64, event: &Event) {
        match event {
            Event::ChannelCreate(_) => self.cache.stats.event_counts.channel_create.inc(),
            Event::ChannelDelete(_) => self.cache.stats.event_counts.channel_delete.inc(),
            Event::GatewayReconnect => self.cache.stats.event_counts.gateway_reconnect.inc(),
            Event::ChannelPinsUpdate(_) => self.cache.stats.event_counts.channel_pins_update.inc(),
            Event::GuildCreate(_) => self.cache.stats.event_counts.guild_create.inc(),
            Event::GuildDelete(_) => self.cache.stats.event_counts.guild_delete.inc(),
            Event::GuildUpdate(_) => self.cache.stats.event_counts.guild_update.inc(),
            Event::MemberAdd(_) => self.cache.stats.event_counts.member_add.inc(),
            Event::MemberRemove(_) => self.cache.stats.event_counts.member_remove.inc(),
            Event::MemberUpdate(_) => self.cache.stats.event_counts.member_update.inc(),
            Event::MemberChunk(_) => self.cache.stats.event_counts.member_chunk.inc(),
            Event::MessageCreate(_) => self.cache.stats.event_counts.message_create.inc(),
            Event::MessageDelete(_) => self.cache.stats.event_counts.message_delete.inc(),
            Event::MessageDeleteBulk(_) => self.cache.stats.event_counts.message_delete_bulk.inc(),
            Event::MessageUpdate(_) => self.cache.stats.event_counts.message_update.inc(),
            Event::ReactionAdd(_) => self.cache.stats.event_counts.reaction_add.inc(),
            Event::ReactionRemove(_) => self.cache.stats.event_counts.reaction_remove.inc(),
            Event::ReactionRemoveAll(_) => self.cache.stats.event_counts.reaction_remove_all.inc(),
            Event::ReactionRemoveEmoji(_) => {
                self.cache.stats.event_counts.reaction_remove_emoji.inc()
            }
            Event::UnavailableGuild(_) => self.cache.stats.event_counts.unavailable_guild.inc(),
            Event::UserUpdate(_) => self.cache.stats.event_counts.user_update.inc(),

            Event::ShardConnecting(_) => self.shard_state_change(shard_id, ShardState::Connecting),
            Event::ShardIdentifying(_) => {
                self.shard_state_change(shard_id, ShardState::Identifying)
            }
            Event::ShardConnected(_) => self.shard_state_change(shard_id, ShardState::Connected),
            Event::Ready(_) => self.shard_state_change(shard_id, ShardState::Ready),
            Event::Resumed => self.shard_state_change(shard_id, ShardState::Ready),
            Event::ShardResuming(_) => self.shard_state_change(shard_id, ShardState::Resuming),
            Event::ShardReconnecting(_) => {
                self.shard_state_change(shard_id, ShardState::Reconnecting)
            }
            Event::ShardDisconnected(_) => {
                self.shard_state_change(shard_id, ShardState::Disconnected)
            }
            _ => {}
        }
    }

    pub fn shard_state_change(&self, shard: u64, new_state: ShardState) {
        if let Some(guard) = self.backend.shard_states.get(&shard) {
            self.get_state_metric(guard.value()).dec();
        }
        info!("Shard {} is now {:?}", shard, new_state);
        self.get_state_metric(&new_state).inc();
        self.backend.shard_states.insert(shard, new_state);
    }

    fn get_state_metric(&self, state: &ShardState) -> &IntGauge {
        match state {
            ShardState::PendingCreation => &self.cache.stats.shard_counts.pending,
            ShardState::Connecting => &self.cache.stats.shard_counts.connecting,
            ShardState::Identifying => &self.cache.stats.shard_counts.identifying,
            ShardState::Connected => &self.cache.stats.shard_counts.connected,
            ShardState::Ready => &self.cache.stats.shard_counts.ready,
            ShardState::Resuming => &self.cache.stats.shard_counts.resuming,
            ShardState::Reconnecting => &self.cache.stats.shard_counts.reconnecting,
            ShardState::Disconnected => &self.cache.stats.shard_counts.disconnected,
        }
    }
}

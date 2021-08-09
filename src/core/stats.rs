use chrono::{DateTime, Utc};
use prometheus::{IntCounter, IntCounterVec, Opts, Registry};
use std::sync::Arc;
use twilight_cache_inmemory::Metrics;

pub struct EventStats {
    pub channel_create: IntCounter,
    pub channel_delete: IntCounter,
    pub channel_update: IntCounter,
    pub gateway_invalidate: IntCounter,
    pub gateway_reconnect: IntCounter,
    pub guild_create: IntCounter,
    pub guild_delete: IntCounter,
    pub guild_update: IntCounter,
    pub interaction_create: IntCounter,
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
    pub role_create: IntCounter,
    pub role_delete: IntCounter,
    pub role_update: IntCounter,
    pub unavailable_guild: IntCounter,
    pub user_update: IntCounter,
}

pub struct MessageCounters {
    pub user_messages: IntCounter,
    pub other_bot_messages: IntCounter,
    pub own_messages: IntCounter,
}

pub struct OsuCounters {
    pub rosu: IntCounterVec,
    pub user_cached: IntCounter,
}

pub struct BotStats {
    pub registry: Registry,
    pub start_time: DateTime<Utc>,
    pub event_counts: EventStats,
    pub message_counts: MessageCounters,
    pub command_counts: IntCounterVec,
    pub osu_metrics: OsuCounters,
    pub cache_metrics: Arc<Metrics>,
}

macro_rules! metric_vec {
    (counter: $opt:literal, $help:literal, $label:literal) => {
        IntCounterVec::new(Opts::new($opt, $help), &[$label]).unwrap();
    };

    (gauge: $opt:literal, $help:literal, $label:literal) => {
        IntGaugeVec::new(Opts::new($opt, $help), &[$label]).unwrap();
    };
}

impl BotStats {
    pub fn new(osu_metrics: IntCounterVec, cache_metrics: Arc<Metrics>) -> Self {
        let event_counter = metric_vec!(counter: "gateway_events", "Gateway events", "events");
        let msg_counter = metric_vec!(counter: "messages", "Received messages", "sender_type");
        let command_counts = metric_vec!(counter: "commands", "Executed commands", "name");

        let registry = Registry::new_custom(Some(String::from("bathbot")), None).unwrap();
        registry.register(Box::new(event_counter.clone())).unwrap();
        registry.register(Box::new(msg_counter.clone())).unwrap();
        registry
            .register(Box::new(cache_metrics.metrics.clone()))
            .unwrap();
        registry.register(Box::new(command_counts.clone())).unwrap();
        registry.register(Box::new(osu_metrics.clone())).unwrap();

        Self {
            registry,
            start_time: Utc::now(),
            event_counts: EventStats {
                channel_create: event_counter.with_label_values(&["ChannelCreate"]),
                channel_delete: event_counter.with_label_values(&["ChannelDelete"]),
                channel_update: event_counter.with_label_values(&["ChannelUpdate"]),
                gateway_invalidate: event_counter.with_label_values(&["GatewayInvalidate"]),
                gateway_reconnect: event_counter.with_label_values(&["GatewayReconnect"]),
                guild_create: event_counter.with_label_values(&["GuildCreate"]),
                guild_delete: event_counter.with_label_values(&["GuildDelete"]),
                guild_update: event_counter.with_label_values(&["GuildUpdate"]),
                interaction_create: event_counter.with_label_values(&["InteractionCreate"]),
                member_add: event_counter.with_label_values(&["MemberAdd"]),
                member_remove: event_counter.with_label_values(&["MemberRemove"]),
                member_update: event_counter.with_label_values(&["MemberUpdate"]),
                member_chunk: event_counter.with_label_values(&["MemberChunk"]),
                message_create: event_counter.with_label_values(&["MessageCreate"]),
                message_delete: event_counter.with_label_values(&["MessageDelete"]),
                message_delete_bulk: event_counter.with_label_values(&["MessageDeleteBulk"]),
                message_update: event_counter.with_label_values(&["MessageUpdate"]),
                reaction_add: event_counter.with_label_values(&["ReactionAdd"]),
                reaction_remove: event_counter.with_label_values(&["ReactionRemove"]),
                reaction_remove_all: event_counter.with_label_values(&["ReactionRemoveAll"]),
                reaction_remove_emoji: event_counter.with_label_values(&["ReactionRemoveEmoji"]),
                role_create: event_counter.with_label_values(&["RoleCreate"]),
                role_delete: event_counter.with_label_values(&["RoleDelete"]),
                role_update: event_counter.with_label_values(&["RoleUpdate"]),
                unavailable_guild: event_counter.with_label_values(&["UnavailableGuild"]),
                user_update: event_counter.with_label_values(&["UserUpdate"]),
            },
            message_counts: MessageCounters {
                user_messages: msg_counter.with_label_values(&["User"]),
                other_bot_messages: msg_counter.with_label_values(&["Bot"]),
                own_messages: msg_counter.with_label_values(&["Own"]),
            },
            command_counts,
            osu_metrics: OsuCounters {
                user_cached: osu_metrics.with_label_values(&["User cached"]),
                rosu: osu_metrics,
            },
            cache_metrics,
        }
    }

    pub fn inc_command(&self, cmd: impl AsRef<str>) {
        self.command_counts.with_label_values(&[cmd.as_ref()]).inc();
    }

    pub fn inc_cached_user(&self) {
        self.osu_metrics.user_cached.inc();
    }
}

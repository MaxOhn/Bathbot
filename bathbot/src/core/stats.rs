use prometheus::{IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry};
use time::OffsetDateTime;

use super::Cache;

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
    pub osutracker_stats_cached: IntCounter,
    pub osutracker_pp_group_cached: IntCounter,
    pub osutracker_id_counts_cached: IntCounter,
    pub osekai_medals_cached: IntCounter,
    pub osekai_badges_cached: IntCounter,
    pub osekai_ranking_cached: IntCounter,
    pub cs_diffs_cached: IntCounter,
    pub pp_ranking_cached: IntCounter,
}

pub struct CommandCounters {
    pub message_commands: IntCounterVec,
    pub slash_commands: IntCounterVec,
    pub components: IntCounterVec,
    pub autocompletes: IntCounterVec,
    pub modals: IntCounterVec,
}

pub struct CacheStats {
    pub guilds: IntGauge,
    pub unavailable_guilds: IntGauge,
    pub members: IntGauge,
    pub users: IntGauge,
    pub roles: IntGauge,
}

pub struct BotStats {
    pub start_time: OffsetDateTime,
    pub event_counts: EventStats,
    pub message_counts: MessageCounters,
    pub command_counts: CommandCounters,
    pub cache_counts: CacheStats,
    pub osu_metrics: OsuCounters,
}

macro_rules! metric_vec {
    (counter: $opt:literal, $help:literal, $label:expr) => {
        IntCounterVec::new(Opts::new($opt, $help), &[$label]).unwrap()
    };

    (gauge: $opt:literal, $help:literal, $label:expr) => {
        IntGaugeVec::new(Opts::new($opt, $help), &[$label]).unwrap()
    };
}

impl BotStats {
    pub fn new(osu_metrics: IntCounterVec) -> (Self, Registry) {
        let event_counter = metric_vec!(counter: "gateway_events", "Gateway events", "events");
        let msg_counter = metric_vec!(counter: "messages", "Received messages", "sender_type");
        let message_commands =
            metric_vec!(counter: "message_commands", "Executed message commands", "name");
        let slash_commands =
            metric_vec!(counter: "slash_commands", "Executed slash commands", "name");
        let components =
            metric_vec!(counter: "components", "Executed interaction components", "name");
        let autocompletes =
            metric_vec!(counter: "autocompletes", "Executed command autocompletes", "name");
        let modals = metric_vec!(counter: "modals", "Executed modals", "name");
        let cache_counter = metric_vec!(gauge: "cache", "Cache counts", "cached_type");

        let registry = Registry::new_custom(Some(String::from("bathbot")), None).unwrap();
        registry.register(Box::new(event_counter.clone())).unwrap();
        registry.register(Box::new(msg_counter.clone())).unwrap();
        registry
            .register(Box::new(message_commands.clone()))
            .unwrap();
        registry.register(Box::new(slash_commands.clone())).unwrap();
        registry.register(Box::new(components.clone())).unwrap();
        registry.register(Box::new(autocompletes.clone())).unwrap();
        registry.register(Box::new(cache_counter.clone())).unwrap();
        registry.register(Box::new(osu_metrics.clone())).unwrap();

        let stats = Self {
            start_time: OffsetDateTime::now_utc(),
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
            command_counts: CommandCounters {
                message_commands,
                slash_commands,
                components,
                autocompletes,
                modals,
            },
            cache_counts: CacheStats {
                guilds: cache_counter.with_label_values(&["Guilds"]),
                unavailable_guilds: cache_counter.with_label_values(&["Unavailable guilds"]),
                members: cache_counter.with_label_values(&["Members"]),
                users: cache_counter.with_label_values(&["Users"]),
                roles: cache_counter.with_label_values(&["Roles"]),
            },
            osu_metrics: OsuCounters {
                user_cached: osu_metrics.with_label_values(&["User cached"]),
                osutracker_stats_cached: osu_metrics
                    .with_label_values(&["osutracker stats cached"]),
                osutracker_pp_group_cached: osu_metrics
                    .with_label_values(&["osutracker pp group cached"]),
                osutracker_id_counts_cached: osu_metrics
                    .with_label_values(&["osutracker id counts cached"]),
                osekai_medals_cached: osu_metrics.with_label_values(&["Medals cached"]),
                osekai_badges_cached: osu_metrics.with_label_values(&["Badges cached"]),
                osekai_ranking_cached: osu_metrics.with_label_values(&["Osekai ranking cached"]),
                pp_ranking_cached: osu_metrics.with_label_values(&["Rankings cached"]),
                cs_diffs_cached: osu_metrics.with_label_values(&["Cached cs difficulties"]),
                rosu: osu_metrics,
            },
        };

        (stats, registry)
    }

    pub fn populate(&self, cache: &Cache) {
        let stats = cache.stats();

        self.cache_counts.guilds.set(stats.guilds() as i64);
        self.cache_counts.members.set(stats.members() as i64);
        self.cache_counts.users.set(stats.users() as i64);
        self.cache_counts.roles.set(stats.roles() as i64);
    }

    pub fn increment_message_command(&self, cmd: &str) {
        self.command_counts
            .message_commands
            .with_label_values(&[cmd])
            .inc();
    }

    pub fn increment_slash_command(&self, cmd: &str) {
        self.command_counts
            .slash_commands
            .with_label_values(&[cmd])
            .inc();
    }

    pub fn increment_component(&self, component: &str) {
        self.command_counts
            .components
            .with_label_values(&[component])
            .inc();
    }

    pub fn increment_autocomplete(&self, cmd: &str) {
        self.command_counts
            .autocompletes
            .with_label_values(&[cmd])
            .inc();
    }

    pub fn increment_modal(&self, modal: &str) {
        self.command_counts.modals.with_label_values(&[modal]).inc();
    }

    pub fn inc_cached_user(&self) {
        self.osu_metrics.user_cached.inc();
    }

    pub fn inc_cached_osutracker_stats(&self) {
        self.osu_metrics.osutracker_stats_cached.inc();
    }

    pub fn inc_cached_osutracker_pp_group(&self) {
        self.osu_metrics.osutracker_pp_group_cached.inc();
    }

    pub fn inc_cached_osutracker_counts(&self) {
        self.osu_metrics.osutracker_id_counts_cached.inc();
    }

    pub fn inc_cached_medals(&self) {
        self.osu_metrics.osekai_medals_cached.inc();
    }

    pub fn inc_cached_badges(&self) {
        self.osu_metrics.osekai_badges_cached.inc();
    }

    pub fn inc_cached_pp_ranking(&self) {
        self.osu_metrics.pp_ranking_cached.inc();
    }

    pub fn inc_cached_osekai_ranking(&self) {
        self.osu_metrics.osekai_ranking_cached.inc();
    }

    pub fn inc_cached_cs_diffs(&self) {
        self.osu_metrics.cs_diffs_cached.inc();
    }
}

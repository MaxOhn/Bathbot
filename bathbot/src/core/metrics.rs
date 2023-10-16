use std::time::Duration;

use bathbot_cache::{model::CacheChange, Cache};
use metrics::{
    describe_counter, describe_gauge, describe_histogram, gauge, histogram, increment_counter,
    increment_gauge, SharedString, Unit,
};
use twilight_gateway::Event;

const GATEWAY_EVENTS: &str = "gateway_events";
const COMMANDS_PROCESS_TIME: &str = "commands_process_time";
const COMMAND_ERRORS: &str = "command_errors";
const CACHE_ENTRIES: &str = "cache_entries";
const REDIS_CACHE_HITS: &str = "redis_cache_hits";

pub struct BotMetrics;

impl BotMetrics {
    pub fn init(cache: &Cache) {
        describe_counter!(GATEWAY_EVENTS, Unit::Count, "Number of gateway events");
        describe_histogram!(
            COMMANDS_PROCESS_TIME,
            Unit::Seconds,
            "Time to process a command in seconds"
        );
        describe_counter!(
            COMMAND_ERRORS,
            Unit::Count,
            "Number of times a command failed"
        );
        describe_gauge!(CACHE_ENTRIES, Unit::Count, "Number of cache entries");
        describe_counter!(
            REDIS_CACHE_HITS,
            Unit::Count,
            "Number of times redis contained a cached value"
        );

        let stats = cache.stats();

        gauge!(CACHE_ENTRIES, stats.guilds as f64, "kind" => "Guilds");
        gauge!(CACHE_ENTRIES, stats.channels as f64, "kind" => "Channels");
        gauge!(CACHE_ENTRIES, stats.users as f64, "kind" => "Users");
        gauge!(CACHE_ENTRIES, stats.roles as f64, "kind" => "Roles");
        gauge!(CACHE_ENTRIES, stats.unavailable_guilds as f64, "kind" => "Unavailable guilds");
    }

    pub fn inc_command_error(kind: &'static str, name: impl Into<SharedString>) {
        increment_counter!(COMMAND_ERRORS, "kind" => kind, "name" => name);
    }

    pub fn inc_slash_command_error(
        name: impl Into<SharedString>,
        group: impl Into<SharedString>,
        sub: impl Into<SharedString>,
    ) {
        increment_counter!(COMMAND_ERRORS,
            "kind" => "slash",
            "name" => name,
            "group" => group,
            "sub" => sub
        );
    }

    pub fn observe_command(kind: &'static str, name: impl Into<SharedString>, duration: Duration) {
        histogram!(COMMANDS_PROCESS_TIME, duration, "kind" => kind, "name" => name);
    }

    pub fn observe_slash_command(
        name: impl Into<SharedString>,
        group: impl Into<SharedString>,
        sub: impl Into<SharedString>,
        duration: Duration,
    ) {
        histogram!(COMMANDS_PROCESS_TIME,
            duration,
            "kind" => "slash",
            "name" => name,
            "group" => group,
            "sub" => sub
        );
    }

    pub fn inc_redis_hit(kind: impl Into<SharedString>) {
        increment_counter!(REDIS_CACHE_HITS, "kind" => kind);
    }

    pub fn event(event: &Event, change: Option<CacheChange>) {
        if let Some(change) = change {
            increment_gauge!(CACHE_ENTRIES, change.guilds as f64, "kind" => "Guilds");
            increment_gauge!(CACHE_ENTRIES, change.channels as f64, "kind" => "Channels");
            increment_gauge!(CACHE_ENTRIES, change.users as f64, "kind" => "Users");
            increment_gauge!(CACHE_ENTRIES, change.roles as f64, "kind" => "Roles");
            increment_gauge!(CACHE_ENTRIES, change.unavailable_guilds as f64, "kind" => "Unavailable guilds");
        }

        if let Some(name) = event.kind().name() {
            increment_counter!(GATEWAY_EVENTS, "event" => name);
        }
    }
}

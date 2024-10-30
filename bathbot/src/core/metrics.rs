use std::time::Duration;

use metrics::{
    counter, describe_counter, describe_gauge, describe_histogram, histogram, SharedString, Unit,
};
use twilight_gateway::Event;

const GATEWAY_EVENTS: &str = "gateway_events";
const COMMANDS_PROCESS_TIME: &str = "commands_process_time";
const COMMAND_ERRORS: &str = "command_errors";
const CACHE_ENTRIES: &str = "cache_entries";
const REDIS_CACHE_HITS: &str = "redis_cache_hits";

pub struct BotMetrics;

impl BotMetrics {
    pub fn init() {
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
    }

    pub fn inc_command_error(kind: &'static str, name: impl Into<SharedString>) {
        counter!(COMMAND_ERRORS, "kind" => kind, "name" => name).increment(1);
    }

    pub fn inc_slash_command_error(
        name: impl Into<SharedString>,
        group: impl Into<SharedString>,
        sub: impl Into<SharedString>,
    ) {
        counter!(COMMAND_ERRORS,
            "kind" => "slash",
            "name" => name,
            "group" => group,
            "sub" => sub
        )
        .increment(1);
    }

    pub fn observe_command(kind: &'static str, name: impl Into<SharedString>, duration: Duration) {
        histogram!(COMMANDS_PROCESS_TIME, "kind" => kind, "name" => name).record(duration);
    }

    pub fn observe_slash_command(
        name: impl Into<SharedString>,
        group: impl Into<SharedString>,
        sub: impl Into<SharedString>,
        duration: Duration,
    ) {
        histogram!(COMMANDS_PROCESS_TIME,
            "kind" => "slash",
            "name" => name,
            "group" => group,
            "sub" => sub
        )
        .record(duration);
    }

    pub fn inc_redis_hit(kind: impl Into<SharedString>) {
        counter!(REDIS_CACHE_HITS, "kind" => kind).increment(1);
    }

    pub fn event(event: &Event) {
        if let Some(name) = event.kind().name() {
            counter!(GATEWAY_EVENTS, "event" => name).increment(1);
        }
    }
}

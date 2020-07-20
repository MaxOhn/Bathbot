mod cache;
mod cold_resume;
mod commands;
mod config;
mod context;
mod handler;
pub mod logging;
mod stats;
mod stored_values;

pub use cache::{Cache, CachedEmoji, CachedUser};
pub use cold_resume::ColdRebootData;
pub use commands::{Command, CommandGroup, CommandGroups};
pub use config::BotConfig;
pub use context::{generate_activity, BackendData, Clients, Context};
pub use handler::handle_event;
pub use stats::BotStats;
pub use stored_values::{StoredValues, Values};

#[derive(PartialEq, Debug)]
pub enum ShardState {
    PendingCreation,
    Connecting,
    Identifying,
    Connected,
    Ready,
    Resuming,
    Reconnecting,
    Disconnected,
}

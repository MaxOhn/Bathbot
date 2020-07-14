mod cache;
mod cold_resume;
mod commands;
mod config;
mod context;
mod handler;
pub mod logging;
mod message_ext;
mod stats;

pub use cache::Cache;
pub use cold_resume::ColdRebootData;
pub use commands::{Command, CommandGroup, CommandGroups};
pub use config::BotConfig;
pub use context::{generate_activity, Context};
pub use handler::handle_event;
pub use message_ext::MessageExt;
pub use stats::BotStats;

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

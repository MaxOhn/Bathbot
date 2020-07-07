pub mod cache;
mod cold_resume_data;
mod config;
mod context;
mod handler;
pub mod logging;
mod stats;

pub use cold_resume_data::ColdRebootData;
pub use config::BotConfig;
pub use context::{generate_activity, Context};
pub use handler::handle_event;
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

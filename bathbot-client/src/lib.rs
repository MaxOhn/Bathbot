#[macro_use]
extern crate eyre;

#[macro_use]
extern crate tracing;

mod client;
mod discord;
mod error;
mod multipart;
mod osu;
mod twitch;

pub use self::{client::Client, error::ClientError};

static MY_USER_AGENT: &str = env!("CARGO_PKG_NAME");

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u8)]
enum Site {
    DiscordAttachment,
    Huismetbenen,
    Osekai,
    OsuAvatar,
    OsuBadge,
    OsuHiddenApi,
    OsuMapFile,
    OsuMapsetCover,
    OsuStats,
    OsuTracker,
    Respektive,
    Twitch,
}

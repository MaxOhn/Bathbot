#[cfg(feature = "osutracking")]
pub use self::osu::{
    osu_loop::{osu_tracking_loop, process_osu_tracking},
    osu_queue::*,
};
#[cfg(feature = "twitch")]
pub use self::twitch::online_streams::OnlineTwitchStreams;
#[cfg(feature = "twitchtracking")]
pub use self::twitch::twitch_loop::twitch_tracking_loop;

mod osu;
mod twitch;

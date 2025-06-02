pub mod fun;
pub mod help;
pub mod osu;
pub mod owner;
pub mod songs;
pub mod tracking;
pub mod utility;

#[cfg(feature = "twitchtracking")]
pub mod twitch;

const DISCORD_OPTION_DESC: &str = "Specify a linked discord user";

const DISCORD_OPTION_HELP: &str = "Instead of specifying an osu! username with \
the `name` option, you can use this option to choose a discord user.\nOnly \
works on users who have used the `/link` command.";

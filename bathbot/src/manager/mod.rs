#[cfg(feature = "twitch")]
pub use self::twitch::TwitchManager;
pub use self::{
    bookmarks::BookmarkManager,
    games::GameManager,
    github::GithubManager,
    guild_config::GuildConfigManager,
    huismetbenen_country::HuismetbenenCountryManager,
    osu_map::{MapError, MapManager, OsuMap, OsuMapSlim},
    osu_scores::ScoresManager,
    osu_user::OsuUserManager,
    pp::{Mods, PpManager},
    rank_pp_approx::ApproxManager,
    replay::{OwnedReplayScore, ReplayManager, ReplayScore, ReplaySettings},
    user_config::UserConfigManager,
};

pub mod redis;

mod bookmarks;
mod games;
mod github;
mod guild_config;
mod huismetbenen_country;
mod osu_map;
mod osu_scores;
mod osu_user;
mod pp;
mod rank_pp_approx;
mod replay;
mod user_config;

#[cfg(feature = "twitch")]
mod twitch;

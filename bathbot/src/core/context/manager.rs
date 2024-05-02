use rosu_pp::Beatmap;
use rosu_v2::prelude::GameMode;

use super::Context;
use crate::manager::{
    redis::RedisManager, ApproxManager, BookmarkManager, GameManager, GithubManager,
    GuildConfigManager, HuismetbenenCountryManager, MapManager, OsuMap, OsuUserManager, PpManager,
    ReplayManager, ScoresManager, UserConfigManager,
};

impl Context {
    pub fn guild_config() -> GuildConfigManager {
        let ctx = Self::get();

        GuildConfigManager::new(&ctx.clients.psql, &ctx.data.guild_configs)
    }

    pub fn user_config() -> UserConfigManager {
        UserConfigManager::new()
    }

    pub fn osu_user() -> OsuUserManager {
        OsuUserManager::new()
    }

    #[cfg(feature = "osutracking")]
    pub fn osu_tracking() -> crate::manager::OsuTrackingManager {
        crate::manager::OsuTrackingManager::new()
    }

    pub fn pp(map: &OsuMap) -> PpManager<'_> {
        PpManager::new(map)
    }

    pub fn pp_parsed(map: &Beatmap, map_id: u32, mode: GameMode) -> PpManager<'_> {
        PpManager::from_parsed(map, map_id).mode(mode)
    }

    pub fn approx() -> ApproxManager {
        ApproxManager::new()
    }

    pub fn games() -> GameManager {
        GameManager::new()
    }

    #[cfg(feature = "twitch")]
    pub fn twitch() -> crate::manager::TwitchManager {
        crate::manager::TwitchManager::new()
    }

    pub fn bookmarks() -> BookmarkManager {
        BookmarkManager::new()
    }

    pub fn replay() -> ReplayManager {
        let ctx = Self::get();

        ReplayManager::new(&ctx.clients.psql, &ctx.clients.custom, &ctx.data.cache)
    }

    pub fn github() -> GithubManager {
        GithubManager::new()
    }

    pub fn redis() -> RedisManager {
        RedisManager::new()
    }

    pub fn osu_map() -> MapManager {
        MapManager::new()
    }

    pub fn osu_scores() -> ScoresManager {
        ScoresManager::new()
    }

    pub fn huismetbenen() -> HuismetbenenCountryManager {
        HuismetbenenCountryManager::new()
    }
}

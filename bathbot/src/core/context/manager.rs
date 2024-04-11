use rosu_pp::Beatmap;
use rosu_v2::prelude::GameMode;

use super::Context;
use crate::manager::{
    ApproxManager, BookmarkManager, GameManager, GithubManager, GuildConfigManager, OsuMap,
    OsuTrackingManager, OsuUserManager, PpManager, ReplayManager, TwitchManager, UserConfigManager,
};

impl Context {
    pub fn guild_config(&self) -> GuildConfigManager<'_> {
        GuildConfigManager::new(&self.clients.psql, &self.data.guild_configs)
    }

    pub fn user_config(&self) -> UserConfigManager<'_> {
        UserConfigManager::new(&self.clients.psql)
    }

    pub fn osu_user(&self) -> OsuUserManager<'_> {
        OsuUserManager::new(&self.clients.psql)
    }

    pub fn osu_tracking(&self) -> OsuTrackingManager<'_> {
        OsuTrackingManager::new(&self.clients.psql)
    }

    pub fn pp<'d, 'm>(&'d self, map: &'m OsuMap) -> PpManager<'d, 'm> {
        PpManager::new(map, &self.clients.psql)
    }

    pub fn pp_parsed<'d, 'm>(
        &'d self,
        map: &'m Beatmap,
        map_id: u32,
        mode: GameMode,
    ) -> PpManager<'d, 'm> {
        PpManager::from_parsed(map, map_id, &self.clients.psql).mode(mode)
    }

    pub fn approx(&self) -> ApproxManager<'_> {
        ApproxManager::new(&self.clients.psql)
    }

    pub fn games(&self) -> GameManager<'_> {
        GameManager::new(&self.clients.psql)
    }

    pub fn twitch(&self) -> TwitchManager<'_> {
        TwitchManager::new(&self.clients.psql)
    }

    pub fn bookmarks(&self) -> BookmarkManager<'_> {
        BookmarkManager::new(&self.clients.psql)
    }

    pub fn replay(&self) -> ReplayManager<'_> {
        ReplayManager::new(&self.clients.psql, &self.clients.custom, &self.cache)
    }

    pub fn github(&self) -> GithubManager<'_> {
        GithubManager::new(self)
    }
}

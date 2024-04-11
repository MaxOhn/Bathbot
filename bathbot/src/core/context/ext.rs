use std::sync::Arc;

use super::Context;
use crate::manager::{redis::RedisManager, HuismetbenenCountryManager, MapManager, ScoresManager};

pub trait ContextExt {
    fn cloned(&self) -> Arc<Context>;

    fn redis(&self) -> RedisManager;

    fn osu_map(&self) -> MapManager;

    fn osu_scores(&self) -> ScoresManager;

    fn huismetbenen(&self) -> HuismetbenenCountryManager;
}

impl ContextExt for Arc<Context> {
    fn cloned(&self) -> Arc<Context> {
        Arc::clone(self)
    }

    fn redis(&self) -> RedisManager {
        RedisManager::new(Arc::clone(self))
    }

    fn osu_map(&self) -> MapManager {
        MapManager::new(Arc::clone(self))
    }

    fn osu_scores(&self) -> ScoresManager {
        ScoresManager::new(Arc::clone(self))
    }

    fn huismetbenen(&self) -> HuismetbenenCountryManager {
        HuismetbenenCountryManager::new(Arc::clone(self))
    }
}

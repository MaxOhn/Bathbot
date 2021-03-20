mod impls;
mod models;
mod util;

pub use models::{
    DBBeatmap, DBBeatmapset, DBOsuMedal, GuildConfig, MapsetTagWrapper, MedalGroup, OsuMedal,
    TagRow, TrackingUser,
};

use crate::BotResult;

use sqlx::postgres::PgPool;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub fn new(uri: &str) -> BotResult<Self> {
        let pool = PgPool::connect_lazy(uri)?;

        Ok(Self { pool })
    }
}

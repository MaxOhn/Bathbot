mod impls;
mod models;
mod util;

pub use models::{BeatmapWrapper, DBMapSet, GuildConfig, MapsetTagWrapper, Ratios, StreamTrack};

use crate::BotResult;

use sqlx::postgres::PgPool;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> BotResult<Self> {
        let pool = PgPool::connect_lazy(database_url)?;
        Ok(Self { pool })
    }
}

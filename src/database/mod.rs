mod impls;
mod models;
pub mod parse;
mod util;

pub use models::{BeatmapWrapper, DBMapSet, GuildConfig, MapsetTagWrapper, Ratios};

use crate::BotResult;

use sqlx::postgres::PgPool;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> BotResult<Self> {
        let pool = PgPool::builder().max_size(16).build(database_url).await?;
        Ok(Self { pool })
    }
}

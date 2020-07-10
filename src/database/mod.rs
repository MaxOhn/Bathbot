mod impls;
mod models;
pub mod parse;
mod util;

pub use models::{BeatmapWrapper, DBMapSet, GuildConfig, MapsetTagWrapper, Ratios};

use crate::BotResult;

use deadpool_postgres::{Manager, Pool};
use std::str::FromStr;
use tokio_postgres::{Config, NoTls};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!();
}

pub struct Database {
    pool: Pool,
}

impl Database {
    pub async fn new(database_url: &str) -> BotResult<Self> {
        let manager = Manager::new(Config::from_str(database_url)?, NoTls);
        let pool = Pool::new(manager, 10);
        let mut connection = pool.get().await?;

        embedded::migrations::runner()
            .run_async(&mut **connection)
            .await?;

        Ok(Self { pool })
    }
}

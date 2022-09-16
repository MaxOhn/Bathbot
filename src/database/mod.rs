mod impls;
mod models;
mod util;

use eyre::Result;
use sqlx::postgres::{PgPool, PgPoolOptions};

pub use self::models::{
    Authorities, DBBeatmap, DBBeatmapset, EmbedsSize, GuildConfig, ListSize, MapsetTagWrapper,
    MinimizedPp, OsuData, Prefix, Prefixes, TagRow, TrackingUser, UserConfig, UserStatsColumn,
    UserValueRaw,
};

pub struct Database {
    pool: PgPool,
}

impl Database {
    #[cold]
    pub fn new(uri: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().connect_lazy(uri)?;

        Ok(Self { pool })
    }
}

mod impls;
mod models;
mod util;

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::BotResult;

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
    pub fn new(uri: &str) -> BotResult<Self> {
        let pool = PgPoolOptions::new().connect_lazy(uri)?;

        Ok(Self { pool })
    }
}

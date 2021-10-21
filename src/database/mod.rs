mod impls;
mod models;
mod util;

pub use models::{
    Authorities, DBBeatmap, DBBeatmapset, DBOsuMedal, GuildConfig, MapsetTagWrapper, MedalGroup,
    OsuData, OsuMedal, Prefix, Prefixes, TagRow, TrackingUser, UserConfig,
};

use crate::BotResult;

use sqlx::postgres::{PgPool, PgPoolOptions};

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

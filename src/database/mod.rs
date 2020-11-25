mod impls;
mod models;
mod util;

pub use models::{BeatmapWrapper, DBMapSet, GuildConfig, MapsetTagWrapper, Ratios, TrackingUser};

use crate::{BotResult, CONFIG};

use sqlx::postgres::PgPool;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(host: &str) -> BotResult<Self> {
        let config = CONFIG.get().unwrap();
        let user = &config.database.db_user;
        let pw = &config.database.db_pw;
        let name = &config.database.db_name;
        let options = sqlx::postgres::PgConnectOptions::new()
            .ssl_mode(sqlx::postgres::PgSslMode::Disable)
            .username(user)
            .password(pw)
            .host(host)
            .database(name);
        let pool = PgPool::connect_lazy_with(options);
        Ok(Self { pool })
    }
}

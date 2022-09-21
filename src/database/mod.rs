use eyre::Result;
use sqlx::postgres::{PgPool, PgPoolOptions};

pub use self::models::*;

mod impls;
mod models;
mod util;

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

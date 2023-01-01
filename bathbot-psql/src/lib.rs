#[macro_use]
extern crate eyre;

#[macro_use]
extern crate tracing;

pub use self::database::Database;

mod database;
mod impls;

pub mod model;

#[cfg(test)]
pub mod tests {
    use eyre::Result;
    use twilight_model::id::Id;

    use super::database::Database;

    pub fn database() -> Result<Database> {
        dotenv::dotenv().unwrap();
        let uri = std::env::var("DATABASE_URL").unwrap();

        Database::new(&uri)
    }

    pub fn discord_id<M>() -> Id<M> {
        Id::new(u64::MAX)
    }

    pub fn osu_user_id() -> u32 {
        2
    }

    pub fn osu_username() -> &'static str {
        "Badewanne3 _dev"
    }
}

mod models;
mod schema;

use models::DBBeatmap;

use crate::util::Error;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    MysqlConnection,
};
use rosu::models::Beatmap;

pub struct MySQL {
    pool: Pool<ConnectionManager<MysqlConnection>>,
}

impl MySQL {
    pub fn new(database_url: &str) -> Self {
        let manager = ConnectionManager::new(database_url);
        let pool = Pool::builder()
            .build(manager)
            .expect("Failed to create pool.");
        Self { pool }
    }

    pub fn insert_beatmap(&self, map: &Beatmap) -> Result<(), Error> {
        let db_map = DBBeatmap::from(map);
        let connection = self
            .pool
            .get()
            .expect("Error while waiting for MySQL connection");
        diesel::insert_into(schema::beatmaps::table)
            .values(&db_map)
            .execute(&connection)?;
        Ok(())
    }

    pub fn get_beatmap(&self, map_id: u32) -> Result<Option<Beatmap>, Error> {
        use schema::beatmaps::dsl::beatmap_id;
        let connection = self
            .pool
            .get()
            .expect("Error while waiting for MySQL connection");
        let db_map = schema::beatmaps::table
            .filter(beatmap_id.eq(map_id))
            .load::<DBBeatmap>(&connection)?
            .pop();
        let map = db_map.map(|m| m.into());
        Ok(map)
    }
}

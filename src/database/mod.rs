mod models;
mod schema;

use models::DBBeatmap;

use crate::util::Error;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    MysqlConnection,
};
use rosu::models::Beatmap;
use std::collections::HashMap;

pub struct MySQL {
    pool: Pool<ConnectionManager<MysqlConnection>>,
}

type ConnectionResult = Result<PooledConnection<ConnectionManager<MysqlConnection>>, Error>;

impl MySQL {
    pub fn new(database_url: &str) -> Result<Self, Error> {
        let manager = ConnectionManager::new(database_url);
        let pool = Pool::builder()
            .build(manager)
            .map_err(|e| err!("Failed to create pool: {}", e))?;
        Ok(Self { pool })
    }

    fn get_connection(&self) -> ConnectionResult {
        self.pool.get().map_err(|e| {
            Error::MySQLConnection(format!("Error while waiting for MySQL connection: {}", e))
        })
    }

    pub fn insert_beatmap(&self, map: &Beatmap) -> Result<(), Error> {
        let db_map = DBBeatmap::from(map);
        let conn = self.get_connection()?;
        diesel::insert_into(schema::beatmaps::table)
            .values(&db_map)
            .execute(&conn)?;
        Ok(())
    }

    pub fn get_beatmap(&self, map_id: u32) -> Result<Option<Beatmap>, Error> {
        use schema::beatmaps::dsl::beatmap_id;
        let conn = self.get_connection()?;
        let db_map = schema::beatmaps::table
            .filter(beatmap_id.eq(map_id))
            .load::<DBBeatmap>(&conn)?
            .pop();
        let map = db_map.map(|m| m.into());
        Ok(map)
    }

    pub fn add_discord_link(&self, discord_id: u64, osu_name: &str) -> Result<(), Error> {
        use schema::discord_users::dsl::{discord_id as id, osu_name as name};
        let entry = vec![(id.eq(discord_id), name.eq(osu_name))];
        let conn = self.get_connection()?;
        diesel::replace_into(schema::discord_users::table)
            .values(&entry)
            .execute(&conn)?;
        Ok(())
    }

    pub fn remove_discord_link(&self, discord_id: u64) -> Result<(), Error> {
        use schema::discord_users::dsl::discord_id as id;
        let conn = self.get_connection()?;
        diesel::delete(schema::discord_users::table.filter(id.eq(discord_id))).execute(&conn)?;
        Ok(())
    }

    pub fn get_discord_links(&self) -> Result<HashMap<u64, String>, Error> {
        let conn = self.get_connection()?;
        let tuples = schema::discord_users::table.load::<(u64, String)>(&conn)?;
        let links: HashMap<u64, String> = tuples.into_iter().collect();
        Ok(links)
    }
}

mod models;
mod schema;

use models::{DBMap, DBMapSet, MapSplit};

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

    // -------------------------------
    // Table: maps / mapsets
    // -------------------------------

    pub fn get_beatmap(&self, map_id: u32) -> Result<Beatmap, Error> {
        use schema::{maps, mapsets};
        let conn = self.get_connection()?;
        let map = maps::table.find(map_id).first::<DBMap>(&conn)?;
        let mapset = mapsets::table
            .find(map.beatmapset_id)
            .first::<DBMapSet>(&conn)?;
        Ok(map.into_beatmap(mapset))
    }

    pub fn insert_beatmap<M>(&self, map: &M) -> Result<(), Error>
    where
        M: MapSplit,
    {
        use schema::{maps, mapsets};
        let (map, mapset) = map.db_split();
        let conn = self.get_connection()?;
        diesel::insert_or_ignore_into(mapsets::table)
            .values(&mapset)
            .execute(&conn)?;
        diesel::insert_into(maps::table)
            .values(&map)
            .execute(&conn)?;
        info!("Inserted beatmap {} into database", map.beatmap_id);
        Ok(())
    }

    // -------------------------------
    // Table: discord_users
    // -------------------------------

    pub fn add_discord_link(&self, discord_id: u64, osu_name: &str) -> Result<(), Error> {
        use schema::discord_users::dsl::{discord_id as id, osu_name as name};
        let entry = vec![(id.eq(discord_id), name.eq(osu_name))];
        let conn = self.get_connection()?;
        diesel::replace_into(schema::discord_users::table)
            .values(&entry)
            .execute(&conn)?;
        info!(
            "Discord user {} now linked to osu name {} in database",
            discord_id, osu_name
        );
        Ok(())
    }

    pub fn remove_discord_link(&self, discord_id: u64) -> Result<(), Error> {
        use schema::discord_users::{self, dsl::discord_id as id};
        let conn = self.get_connection()?;
        diesel::delete(discord_users::table.filter(id.eq(discord_id))).execute(&conn)?;
        Ok(())
    }

    pub fn get_discord_links(&self) -> Result<HashMap<u64, String>, Error> {
        use schema::discord_users;
        let conn = self.get_connection()?;
        let tuples = discord_users::table.load::<(u64, String)>(&conn)?;
        let links: HashMap<u64, String> = tuples.into_iter().collect();
        Ok(links)
    }
}

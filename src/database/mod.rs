mod models;
mod schema;

use models::{DBMap, DBMapSet, ManiaPP, MapSplit};

use crate::util::Error;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    MysqlConnection,
};
use rosu::models::{Beatmap, GameMod, GameMods};
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

    pub fn get_beatmaps(&self, map_ids: &[u32]) -> Result<HashMap<u32, Beatmap>, Error> {
        if map_ids.is_empty() {
            return Ok(HashMap::new());
        }
        use schema::{
            maps::{self, dsl::beatmap_id},
            mapsets::{self, dsl::beatmapset_id},
        };
        let conn = self.get_connection()?;
        // Retrieve all DBMap's
        let mut maps: Vec<DBMap> = maps::table
            .filter(beatmap_id.eq_any(map_ids))
            .load::<DBMap>(&conn)?;
        // Sort them by beatmapset_id
        maps.sort_by(|a, b| a.beatmapset_id.cmp(&b.beatmapset_id));
        // Check if all maps are from different mapsets by removing duplicates
        let mut mapset_ids: Vec<_> = maps.iter().map(|m| m.beatmapset_id).collect();
        mapset_ids.dedup();
        //println!("Mapset_ids ({}): {:?}", mapset_ids.len(), mapset_ids);
        // Retrieve all DBMapSet's
        let mut mapsets: Vec<DBMapSet> = mapsets::table
            .filter(beatmapset_id.eq_any(&mapset_ids))
            .load::<DBMapSet>(&conn)?;
        // If all maps have different mapsets
        let beatmaps = if maps.len() == mapset_ids.len() {
            // Sort DBMapSet's by beatmapset'd
            mapsets.sort_by(|a, b| a.beatmapset_id.cmp(&b.beatmapset_id));
            // Then zip them with the DBMap's
            maps.into_iter()
                .zip(mapsets.into_iter())
                .map(|(m, ms)| (m.beatmap_id, m.into_beatmap(ms)))
                .collect()
        // Otherwise (some maps are from the same mapset)
        } else {
            // Collect mapsets into HashMap
            let mapsets: HashMap<u32, DBMapSet> = mapsets
                .into_iter()
                .map(|ms| (ms.beatmapset_id, ms))
                .collect();
            // Clone mapset for each corresponding map
            maps.into_iter()
                .map(|m| {
                    let mapset: DBMapSet = mapsets.get(&m.beatmapset_id).unwrap().clone();
                    let map = m.into_beatmap(mapset);
                    (map.beatmap_id, map)
                })
                .collect()
        };
        Ok(beatmaps)
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
        diesel::insert_or_ignore_into(maps::table)
            .values(&map)
            .execute(&conn)?;
        info!("Inserted beatmap {} into database", map.beatmap_id);
        Ok(())
    }

    pub fn insert_beatmaps<M>(&self, maps: Vec<M>) -> Result<(), Error>
    where
        M: MapSplit,
    {
        use schema::{maps, mapsets};
        let (maps, mapsets): (Vec<DBMap>, Vec<DBMapSet>) =
            maps.into_iter().map(|m| m.into_db_split()).unzip();
        let conn = self.get_connection()?;
        diesel::insert_or_ignore_into(mapsets::table)
            .values(&mapsets)
            .execute(&conn)?;
        diesel::insert_or_ignore_into(maps::table)
            .values(&maps)
            .execute(&conn)?;
        let map_ids: Vec<u32> = maps.iter().map(|m| m.beatmap_id).collect();
        if map_ids.len() > 5 {
            info!("Inserted {} beatmaps into database", map_ids.len());
        } else {
            info!("Inserted beatmaps {:?} into database", map_ids);
        }
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

    // -------------------------------
    // Table: pp_mania_mods
    // -------------------------------

    pub fn get_mania_mod_pp(&self, map_id: u32, mods: &GameMods) -> Result<Option<f32>, Error> {
        let bits = mania_mod_bits(mods);
        let conn = self.get_connection()?;
        let data = schema::pp_mania_mods::table
            .find(map_id)
            .first::<ManiaPP>(&conn)?;
        data.get(bits)
    }

    pub fn insert_mania_pp_map(&self, map_id: u32, mods: &GameMods, pp: f32) -> Result<(), Error> {
        let bits = mania_mod_bits(mods);
        let data = ManiaPP::new(map_id, bits, Some(pp))?;
        let conn = self.get_connection()?;
        diesel::insert_or_ignore_into(schema::pp_mania_mods::table)
            .values(&data)
            .execute(&conn)?;
        info!("Inserted map id {} into pp_mania_mods table", map_id);
        Ok(())
    }

    pub fn update_mania_pp_map(&self, map_id: u32, mods: &GameMods, pp: f32) -> Result<(), Error> {
        use schema::pp_mania_mods::{self, columns::*};
        let bits = mania_mod_bits(mods);
        let conn = self.get_connection()?;
        let query = diesel::update(pp_mania_mods::table.filter(beatmap_id.eq(map_id)));
        match bits {
            0 => query.set(NM.eq(pp)).execute(&conn)?,
            1 => query.set(NF.eq(pp)).execute(&conn)?,
            2 => query.set(EZ.eq(pp)).execute(&conn)?,
            3 => query.set(NFEZ.eq(pp)).execute(&conn)?,
            64 => query.set(DT.eq(pp)).execute(&conn)?,
            65 => query.set(NFDT.eq(pp)).execute(&conn)?,
            66 => query.set(EZDT.eq(pp)).execute(&conn)?,
            67 => query.set(NFEZDT.eq(pp)).execute(&conn)?,
            256 => query.set(HT.eq(pp)).execute(&conn)?,
            257 => query.set(NFHT.eq(pp)).execute(&conn)?,
            258 => query.set(EZHT.eq(pp)).execute(&conn)?,
            259 => query.set(NFEZHT.eq(pp)).execute(&conn)?,
            _ => {
                return Err(Error::Custom(format!(
                    "{} are no valid mod bits for the mania pp table",
                    bits
                )));
            }
        };
        info!(
            "Updated map id {} with mods {} in pp_mania_mods table",
            map_id, mods
        );
        Ok(())
    }
}

fn mania_mod_bits(mods: &GameMods) -> u32 {
    use GameMod::{DoubleTime, Easy, HalfTime, NightCore, NoFail};
    let mut bits = 0;
    for &m in mods.iter() {
        match m {
            NightCore => bits += DoubleTime as u32,
            DoubleTime | Easy | HalfTime | NoFail => bits += m as u32,
            _ => {}
        }
    }
    bits
}

mod models;
mod schema;

use models::{DBMap, DBMapSet, GuildDB, ManiaPP, MapSplit, StreamTrackDB};
pub use models::{Guild, Platform, StreamTrack, TwitchUser};

use crate::util::{globals::AUTHORITY_ROLES, Error};
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    MysqlConnection,
};
use rosu::models::{Beatmap, GameMod, GameMods};
use serenity::model::id::{GuildId, UserId};
use std::collections::{HashMap, HashSet};

pub struct MySQL {
    pool: Pool<ConnectionManager<MysqlConnection>>,
}

type ConnectionResult = Result<PooledConnection<ConnectionManager<MysqlConnection>>, Error>;
type DBResult<T> = Result<T, Error>;

impl MySQL {
    pub fn new(database_url: &str) -> DBResult<Self> {
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

    // ---------------------
    // Table: maps / mapsets
    // ---------------------

    pub fn get_beatmap(&self, map_id: u32) -> DBResult<Beatmap> {
        use schema::{maps, mapsets};
        let conn = self.get_connection()?;
        let map = maps::table.find(map_id).first::<DBMap>(&conn)?;
        let mapset = mapsets::table
            .find(map.beatmapset_id)
            .first::<DBMapSet>(&conn)?;
        Ok(map.into_beatmap(mapset))
    }

    pub fn get_beatmaps(&self, map_ids: &[u32]) -> DBResult<HashMap<u32, Beatmap>> {
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

    pub fn insert_beatmap<M>(&self, map: &M) -> DBResult<()>
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

    pub fn insert_beatmaps<M>(&self, maps: Vec<M>) -> DBResult<()>
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

    // --------------------
    // Table: discord_users
    // --------------------

    pub fn add_discord_link(&self, discord_id: u64, osu_name: &str) -> DBResult<()> {
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
        let conn = self.get_connection()?;
        let tuples = schema::discord_users::table.load::<(u64, String)>(&conn)?;
        let links: HashMap<u64, String> = tuples.into_iter().collect();
        Ok(links)
    }

    // --------------------
    // Table: pp_mania_mods
    // --------------------

    pub fn get_mania_mod_pp(&self, map_id: u32, mods: &GameMods) -> DBResult<Option<f32>> {
        let bits = mania_mod_bits(mods);
        let conn = self.get_connection()?;
        let data = schema::pp_mania_mods::table
            .find(map_id)
            .first::<ManiaPP>(&conn)?;
        data.get(bits)
    }

    pub fn insert_mania_pp_map(&self, map_id: u32, mods: &GameMods, pp: f32) -> DBResult<()> {
        let bits = mania_mod_bits(mods);
        let data = ManiaPP::new(map_id, bits, Some(pp))?;
        let conn = self.get_connection()?;
        diesel::insert_or_ignore_into(schema::pp_mania_mods::table)
            .values(&data)
            .execute(&conn)?;
        info!("Inserted map id {} into pp_mania_mods table", map_id);
        Ok(())
    }

    pub fn update_mania_pp_map(&self, map_id: u32, mods: &GameMods, pp: f32) -> DBResult<()> {
        use schema::pp_mania_mods::{self, columns::*};
        let bits = mania_mod_bits(mods);
        let conn = self.get_connection()?;
        let data = ManiaPP::new(map_id, bits, Some(pp))?;
        diesel::update(pp_mania_mods::table.filter(beatmap_id.eq(map_id)))
            .set(&data)
            .execute(&conn)?;
        info!(
            "Updated map id {} with mods {} in pp_mania_mods table",
            map_id, mods
        );
        Ok(())
    }

    // ------------------
    // Table: role_assign
    // ------------------

    pub fn get_role_assigns(&self) -> DBResult<HashMap<(u64, u64), u64>> {
        let conn = self.get_connection()?;
        let tuples = schema::role_assign::table.load::<(u32, u64, u64, u64)>(&conn)?;
        let map = tuples.into_iter().map(|(_, c, m, r)| ((c, m), r)).collect();
        Ok(map)
    }

    pub fn add_role_assign(&self, channel_id: u64, message_id: u64, role_id: u64) -> DBResult<()> {
        use schema::role_assign::dsl::{channel, message, role};
        let conn = self.get_connection()?;
        diesel::insert_into(schema::role_assign::table)
            .values((
                channel.eq(channel_id),
                message.eq(message_id),
                role.eq(role_id),
            ))
            .execute(&conn)?;
        info!("Inserted into role_assign table");
        Ok(())
    }

    // -----------------------------------
    // Table: stream_tracks / twitch_users
    // -----------------------------------

    pub fn add_twitch_user(&self, id: u64, username: &str) -> DBResult<()> {
        use schema::twitch_users::dsl::{name, user_id};
        let conn = self.get_connection()?;
        diesel::insert_into(schema::twitch_users::table)
            .values((user_id.eq(id), name.eq(username)))
            .execute(&conn)?;
        info!("Inserted into twitch_users table");
        Ok(())
    }

    pub fn add_stream_track(&self, channel: u64, user: u64, pf: Platform) -> DBResult<()> {
        use schema::stream_tracks::dsl::{channel_id, platform, user_id};
        let conn = self.get_connection()?;
        diesel::insert_into(schema::stream_tracks::table)
            .values((
                channel_id.eq(channel),
                user_id.eq(user),
                platform.eq(pf as u8),
            ))
            .execute(&conn)?;
        info!("Inserted into stream_tracks table");
        Ok(())
    }

    pub fn get_twitch_users(&self) -> DBResult<HashMap<String, u64>> {
        let conn = self.get_connection()?;
        let tuples = schema::twitch_users::table.load::<(u64, String)>(&conn)?;
        let users: HashMap<_, _> = tuples.into_iter().map(|(id, name)| (name, id)).collect();
        Ok(users)
    }

    pub fn get_stream_tracks(&self) -> DBResult<HashSet<StreamTrack>> {
        let conn = self.get_connection()?;
        let tracks = schema::stream_tracks::table.load::<StreamTrackDB>(&conn)?;
        let tracks = tracks.into_iter().map(StreamTrackDB::into).collect();
        Ok(tracks)
    }

    pub fn remove_stream_track(&self, channel: u64, user: u64, pf: Platform) -> DBResult<()> {
        use schema::stream_tracks::columns;
        let conn = self.get_connection()?;
        diesel::delete(
            schema::stream_tracks::table
                .filter(columns::channel_id.eq(channel))
                .filter(columns::user_id.eq(user))
                .filter(columns::platform.eq(pf as u8)),
        )
        .execute(&conn)?;
        Ok(())
    }

    // -----------------------------------
    // Table: stream_tracks / twitch_users
    // -----------------------------------

    pub fn get_guilds(&self) -> DBResult<HashMap<GuildId, Guild>> {
        let conn = self.get_connection()?;
        let guilds = schema::guilds::table.load::<GuildDB>(&conn)?;
        let guilds = guilds
            .into_iter()
            .map(|g| (GuildId(g.guild_id), g.into()))
            .collect();
        Ok(guilds)
    }

    pub fn insert_guild(&self, guild_id: u64) -> DBResult<Guild> {
        let guild = GuildDB::new(guild_id, true, AUTHORITY_ROLES.to_string(), None);
        let conn = self.get_connection()?;
        diesel::insert_or_ignore_into(schema::guilds::table)
            .values(&guild)
            .execute(&conn)?;
        info!("Inserted new guild id {} into database", guild_id);
        Ok(guild.into())
    }

    pub fn update_guild_vc_role(&self, guild: u64, role_id: Option<u64>) -> DBResult<()> {
        use schema::guilds::columns::{guild_id, vc_role};
        let conn = self.get_connection()?;
        let target = schema::guilds::table.filter(guild_id.eq(guild));
        diesel::update(target)
            .set(vc_role.eq(role_id))
            .execute(&conn)?;
        info!("Updated VC role for guild id {}", guild);
        Ok(())
    }

    pub fn update_guild_lyrics(&self, guild: u64, lyrics: bool) -> DBResult<()> {
        use schema::guilds::columns::{guild_id, with_lyrics};
        let conn = self.get_connection()?;
        let target = schema::guilds::table.filter(guild_id.eq(guild));
        diesel::update(target)
            .set(with_lyrics.eq(lyrics))
            .execute(&conn)?;
        info!("Updated lyrics for guild id {}", guild);
        Ok(())
    }

    pub fn update_guild_authorities(&self, guild: u64, auths: String) -> DBResult<()> {
        use schema::guilds::columns::{authorities, guild_id};
        let conn = self.get_connection()?;
        let target = schema::guilds::table.filter(guild_id.eq(guild));
        diesel::update(target)
            .set(authorities.eq(auths))
            .execute(&conn)?;
        info!("Updated authorities for guild id {}", guild);
        Ok(())
    }

    // ------------------------
    // Table: unchecked_members
    // ------------------------

    pub fn get_unchecked_members(&self) -> DBResult<HashMap<UserId, DateTime<Utc>>> {
        let conn = self.get_connection()?;
        let members = schema::unchecked_members::table.load::<(u64, NaiveDateTime)>(&conn)?;
        let members = members
            .into_iter()
            .map(|(id, date)| (UserId(id), DateTime::from_utc(date, Utc)))
            .collect();
        Ok(members)
    }

    pub fn insert_unchecked_member(&self, user: u64, date: DateTime<Utc>) -> DBResult<()> {
        use schema::unchecked_members::columns::{joined, user_id};
        let conn = self.get_connection()?;
        diesel::insert_into(schema::unchecked_members::table)
            .values((user_id.eq(user), joined.eq(date.naive_utc())))
            .execute(&conn)?;
        info!("Inserted unchecked member {} into database", user);
        Ok(())
    }

    pub fn remove_unchecked_member(&self, user: u64) -> DBResult<()> {
        use schema::unchecked_members::{self, columns::user_id};
        let conn = self.get_connection()?;
        diesel::delete(unchecked_members::table.filter(user_id.eq(user))).execute(&conn)?;
        info!("Removed unchecked member {} from database", user);
        Ok(())
    }

    // ------------------------
    // Table: manual_links
    // ------------------------

    pub fn get_manual_links(&self) -> Result<HashMap<u64, String>, Error> {
        let conn = self.get_connection()?;
        let tuples = schema::manual_links::table.load::<(u64, String)>(&conn)?;
        let links: HashMap<u64, String> = tuples.into_iter().collect();
        Ok(links)
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

mod models;
mod schema;

use models::{CtbPP, DBMap, GuildDB, ManiaPP, MapsetTagDB, StreamTrackDB};
pub use models::{
    DBMapSet, Guild, MapSplit, MapsetTagWrapper, Platform, Ratios, StreamTrack, TwitchUser,
};

use crate::{
    commands::utility::MapsetTags,
    util::{globals::AUTHORITY_ROLES, Error},
};

use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    sql_types::Text,
    MysqlConnection,
};
use rosu::models::{Beatmap, GameMode, GameMods};
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
            .map_err(|e| Error::Custom(format!("Failed to create pool: {}", e)))?;
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

    pub fn get_beatmapset(&self, mapset_id: u32) -> DBResult<DBMapSet> {
        use schema::mapsets;
        let conn = self.get_connection()?;
        let mapset = mapsets::table.find(mapset_id).first::<DBMapSet>(&conn)?;
        Ok(mapset)
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
        debug!("Inserted beatmap {} into DB", map.beatmap_id);
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
            debug!("Inserted {} beatmaps into DB", map_ids.len());
        } else {
            debug!("Inserted beatmaps {:?} into DB", map_ids);
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

    // ----------------------------------
    // Table: pp_mania_mods / pp_ctb_mods
    // ----------------------------------

    pub fn get_mod_pp(&self, map_id: u32, mode: GameMode, mods: GameMods) -> DBResult<Option<f32>> {
        let conn = self.get_connection()?;
        if mode == GameMode::MNA {
            let bits = mania_mod_bits(mods);
            schema::pp_mania_mods::table
                .find(map_id)
                .first::<ManiaPP>(&conn)?
                .get(bits)
        } else {
            let data = schema::pp_ctb_mods::table
                .find(map_id)
                .first::<CtbPP>(&conn)?;
            if mods.is_empty() {
                Ok(data.NM)
            } else {
                match mods {
                    GameMods::Hidden => Ok(data.HD),
                    GameMods::HardRock => Ok(data.HR),
                    GameMods::DoubleTime | GameMods::NightCore => Ok(data.DT),
                    m if m == GameMods::from_bits(24).unwrap() => Ok(data.HDHR),
                    m if m == GameMods::from_bits(72).unwrap()
                        || m == GameMods::NightCore | GameMods::Hidden =>
                    {
                        Ok(data.HDDT)
                    }
                    _ => Ok(None),
                }
            }
        }
    }

    pub fn insert_pp_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mods: GameMods,
        pp: f32,
    ) -> DBResult<()> {
        let conn = self.get_connection()?;
        if mode == GameMode::MNA {
            let bits = mania_mod_bits(mods);
            let data = ManiaPP::new(map_id, bits, Some(pp))?;
            diesel::insert_or_ignore_into(schema::pp_mania_mods::table)
                .values(&data)
                .execute(&conn)?;
        } else {
            let mut data = CtbPP::default();
            data.beatmap_id = map_id;
            if mods.is_empty() {
                data.NM = Some(pp);
            } else {
                match mods {
                    GameMods::Hidden => data.HD = Some(pp),
                    GameMods::HardRock => data.HR = Some(pp),
                    GameMods::DoubleTime | GameMods::NightCore => data.DT = Some(pp),
                    m if m == GameMods::from_bits(24).unwrap() => data.HDHR = Some(pp),
                    m if m == GameMods::from_bits(72).unwrap()
                        || m == GameMods::NightCore | GameMods::Hidden =>
                    {
                        data.HDDT = Some(pp)
                    }
                    _ => return Ok(()),
                }
            }
            diesel::insert_or_ignore_into(schema::pp_ctb_mods::table)
                .values(&data)
                .execute(&conn)?;
        };
        Ok(())
    }

    pub fn update_pp_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mods: GameMods,
        pp: f32,
    ) -> DBResult<()> {
        let conn = self.get_connection()?;
        if mode == GameMode::MNA {
            use schema::pp_mania_mods::{self, columns::beatmap_id};
            let bits = mania_mod_bits(mods);
            let data = ManiaPP::new(map_id, bits, Some(pp))?;
            diesel::update(pp_mania_mods::table.filter(beatmap_id.eq(map_id)))
                .set(&data)
                .execute(&conn)?;
        } else {
            use schema::pp_ctb_mods::{self, columns::beatmap_id};

            let mut data = CtbPP::default();
            data.beatmap_id = map_id;
            if mods.is_empty() {
                data.NM = Some(pp);
            } else {
                match mods {
                    GameMods::Hidden => data.HD = Some(pp),
                    GameMods::HardRock => data.HR = Some(pp),
                    GameMods::DoubleTime | GameMods::NightCore => data.DT = Some(pp),
                    m if m == GameMods::from_bits(24).unwrap() => data.HDHR = Some(pp),
                    m if m == GameMods::from_bits(72).unwrap()
                        || m == GameMods::NightCore | GameMods::Hidden =>
                    {
                        data.HDDT = Some(pp)
                    }
                    _ => return Ok(()),
                }
            }
            diesel::update(pp_ctb_mods::table.filter(beatmap_id.eq(map_id)))
                .set(&data)
                .execute(&conn)?;
        }
        Ok(())
    }

    // ----------------------------------------
    // Table: stars_mania_mods / stars_ctb_mods
    // ----------------------------------------

    pub fn get_mod_stars(
        &self,
        map_id: u32,
        mode: GameMode,
        mods: GameMods,
    ) -> DBResult<Option<f32>> {
        let conn = self.get_connection()?;
        if mode == GameMode::MNA {
            let data =
                schema::stars_mania_mods::table
                    .find(map_id)
                    .first::<(u32, Option<f32>, Option<f32>)>(&conn)?;
            if mods.contains(GameMods::DoubleTime) {
                Ok(data.1)
            } else if mods.contains(GameMods::HalfTime) {
                Ok(data.2)
            } else {
                Ok(None)
            }
        } else {
            let data = schema::stars_ctb_mods::table.find(map_id).first::<(
                u32,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
            )>(&conn)?;
            if mods.contains(GameMods::Easy) {
                if mods.contains(GameMods::DoubleTime) {
                    Ok(data.5)
                } else if mods.contains(GameMods::HalfTime) {
                    Ok(data.7)
                } else {
                    Ok(data.1)
                }
            } else if mods.contains(GameMods::HardRock) {
                if mods.contains(GameMods::DoubleTime) {
                    Ok(data.6)
                } else if mods.contains(GameMods::HalfTime) {
                    Ok(data.8)
                } else {
                    Ok(data.2)
                }
            } else if mods.contains(GameMods::DoubleTime) {
                Ok(data.3)
            } else if mods.contains(GameMods::HalfTime) {
                Ok(data.4)
            } else {
                panic!("Don't call update_stars_map with CtB on NoMod");
            }
        }
    }

    pub fn insert_stars_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mods: GameMods,
        stars: f32,
    ) -> DBResult<()> {
        use schema::{
            stars_ctb_mods::columns::{
                beatmap_id as cID, DT as cDT, EZ as cEZ, EZDT as cEZDT, EZHT as cEZHT, HR as cHR,
                HRDT as cHRDT, HRHT as cHRHT, HT as cHT,
            },
            stars_mania_mods::columns::{beatmap_id as mID, DT as mDT, HT as mHT},
        };
        let conn = self.get_connection()?;
        if mode == GameMode::MNA {
            let data = if mods.contains(GameMods::DoubleTime) {
                (mID.eq(map_id), mDT.eq(Some(stars)), mHT.eq(None))
            } else if mods.contains(GameMods::HalfTime) {
                (mID.eq(map_id), mDT.eq(None), mHT.eq(Some(stars)))
            } else {
                (mID.eq(map_id), mDT.eq(None), mHT.eq(None))
            };
            diesel::insert_or_ignore_into(schema::stars_mania_mods::table)
                .values(&data)
                .execute(&conn)?;
        } else {
            let data = if mods.contains(GameMods::Easy) {
                if mods.contains(GameMods::DoubleTime) {
                    (
                        cID.eq(map_id),
                        cEZ.eq(None),
                        cHR.eq(None),
                        cDT.eq(None),
                        cHT.eq(None),
                        cEZDT.eq(Some(stars)),
                        cHRDT.eq(None),
                        cEZHT.eq(None),
                        cHRHT.eq(None),
                    )
                } else if mods.contains(GameMods::HalfTime) {
                    (
                        cID.eq(map_id),
                        cEZ.eq(None),
                        cHR.eq(None),
                        cDT.eq(None),
                        cHT.eq(None),
                        cEZDT.eq(None),
                        cHRDT.eq(None),
                        cEZHT.eq(Some(stars)),
                        cHRHT.eq(None),
                    )
                } else {
                    (
                        cID.eq(map_id),
                        cEZ.eq(Some(stars)),
                        cHR.eq(None),
                        cDT.eq(None),
                        cHT.eq(None),
                        cEZDT.eq(None),
                        cHRDT.eq(None),
                        cEZHT.eq(None),
                        cHRHT.eq(None),
                    )
                }
            } else if mods.contains(GameMods::HardRock) {
                if mods.contains(GameMods::DoubleTime) {
                    (
                        cID.eq(map_id),
                        cEZ.eq(None),
                        cHR.eq(None),
                        cDT.eq(None),
                        cHT.eq(None),
                        cEZDT.eq(None),
                        cHRDT.eq(Some(stars)),
                        cEZHT.eq(None),
                        cHRHT.eq(None),
                    )
                } else if mods.contains(GameMods::HalfTime) {
                    (
                        cID.eq(map_id),
                        cEZ.eq(None),
                        cHR.eq(None),
                        cDT.eq(None),
                        cHT.eq(None),
                        cEZDT.eq(None),
                        cHRDT.eq(None),
                        cEZHT.eq(None),
                        cHRHT.eq(Some(stars)),
                    )
                } else {
                    (
                        cID.eq(map_id),
                        cEZ.eq(None),
                        cHR.eq(Some(stars)),
                        cDT.eq(None),
                        cHT.eq(None),
                        cEZDT.eq(None),
                        cHRDT.eq(None),
                        cEZHT.eq(None),
                        cHRHT.eq(None),
                    )
                }
            } else if mods.contains(GameMods::DoubleTime) {
                (
                    cID.eq(map_id),
                    cEZ.eq(None),
                    cHR.eq(None),
                    cDT.eq(Some(stars)),
                    cHT.eq(None),
                    cEZDT.eq(None),
                    cHRDT.eq(None),
                    cEZHT.eq(None),
                    cHRHT.eq(None),
                )
            } else if mods.contains(GameMods::HalfTime) {
                (
                    cID.eq(map_id),
                    cEZ.eq(None),
                    cHR.eq(None),
                    cDT.eq(None),
                    cHT.eq(Some(stars)),
                    cEZDT.eq(None),
                    cHRDT.eq(None),
                    cEZHT.eq(None),
                    cHRHT.eq(None),
                )
            } else {
                panic!("Don't call insert_stars_map with CtB on NoMod")
            };
            diesel::insert_or_ignore_into(schema::stars_ctb_mods::table)
                .values(&data)
                .execute(&conn)?;
        }
        Ok(())
    }

    pub fn update_stars_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mods: GameMods,
        stars: f32,
    ) -> DBResult<()> {
        use schema::{
            stars_ctb_mods::columns::{
                beatmap_id as cID, DT as cDT, EZ as cEZ, EZDT as cEZDT, EZHT as cEZHT, HR as cHR,
                HRDT as cHRDT, HRHT as cHRHT, HT as cHT,
            },
            stars_mania_mods::columns::{beatmap_id as mID, DT as mDT, HT as mHT},
        };
        let conn = self.get_connection()?;
        if mode == GameMode::MNA {
            let update = diesel::update(schema::stars_mania_mods::table.filter(mID.eq(map_id)));
            if mods.contains(GameMods::DoubleTime) {
                update.set(mDT.eq(Some(stars))).execute(&conn)?;
            } else if mods.contains(GameMods::HalfTime) {
                update.set(mHT.eq(Some(stars))).execute(&conn)?;
            };
        } else {
            let update = diesel::update(schema::stars_ctb_mods::table.filter(cID.eq(map_id)));
            if mods.contains(GameMods::Easy) {
                if mods.contains(GameMods::DoubleTime) {
                    update.set(cEZDT.eq(Some(stars))).execute(&conn)?;
                } else if mods.contains(GameMods::HalfTime) {
                    update.set(cEZHT.eq(Some(stars))).execute(&conn)?;
                } else {
                    update.set(cEZ.eq(Some(stars))).execute(&conn)?;
                }
            } else if mods.contains(GameMods::HardRock) {
                if mods.contains(GameMods::DoubleTime) {
                    update.set(cHRDT.eq(Some(stars))).execute(&conn)?;
                } else if mods.contains(GameMods::HalfTime) {
                    update.set(cHRHT.eq(Some(stars))).execute(&conn)?;
                } else {
                    update.set(cHR.eq(Some(stars))).execute(&conn)?;
                }
            } else if mods.contains(GameMods::DoubleTime) {
                update.set(cDT.eq(Some(stars))).execute(&conn)?;
            } else if mods.contains(GameMods::HalfTime) {
                update.set(cHT.eq(Some(stars))).execute(&conn)?;
            } else {
                panic!("Don't call update_stars_map with CtB on NoMod");
            }
        };
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

    // -------------
    // Table: guilds
    // -------------

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
        let guild = GuildDB::new(guild_id, true, AUTHORITY_ROLES.to_string());
        let conn = self.get_connection()?;
        diesel::insert_or_ignore_into(schema::guilds::table)
            .values(&guild)
            .execute(&conn)?;
        Ok(guild.into())
    }

    pub fn update_guild_lyrics(&self, guild: u64, lyrics: bool) -> DBResult<()> {
        use schema::guilds::columns::{guild_id, with_lyrics};
        let conn = self.get_connection()?;
        let target = schema::guilds::table.filter(guild_id.eq(guild));
        diesel::update(target)
            .set(with_lyrics.eq(lyrics))
            .execute(&conn)?;
        Ok(())
    }

    pub fn update_guild_authorities(&self, guild: u64, auths: String) -> DBResult<()> {
        use schema::guilds::columns::{authorities, guild_id};
        let conn = self.get_connection()?;
        let target = schema::guilds::table.filter(guild_id.eq(guild));
        diesel::update(target)
            .set(authorities.eq(auths))
            .execute(&conn)?;
        Ok(())
    }

    // -------------------
    // Table: bggame_stats
    // -------------------

    pub fn increment_bggame_score(&self, user: u64) -> DBResult<()> {
        let conn = self.get_connection()?;
        let query = format!(
            "INSERT INTO bggame_stats(discord_id, score) values ({}, 1) \
            on duplicate key update score = score + 1",
            user
        );
        diesel::sql_query(query).execute(&conn)?;
        Ok(())
    }

    pub fn get_bggame_score(&self, user: u64) -> DBResult<u32> {
        let conn = self.get_connection()?;
        let data = schema::bggame_stats::table
            .find(user)
            .first::<(u64, u32)>(&conn)?;
        Ok(data.1)
    }

    pub fn all_bggame_scores(&self) -> DBResult<Vec<(u64, u32)>> {
        let conn = self.get_connection()?;
        Ok(schema::bggame_stats::table.load(&conn)?)
    }

    // ------------------
    // Table: ratio_table
    // ------------------

    pub fn update_ratios(
        &self,
        osuname: &str,
        all_scores: String,
        all_ratios: String,
        all_misses: String,
    ) -> Option<Ratios> {
        use schema::ratio_table::columns::{misses, name, ratios, scores};
        let entry = vec![(
            name.eq(osuname),
            scores.eq(all_scores),
            ratios.eq(all_ratios),
            misses.eq(all_misses),
        )];
        let conn = if let Ok(conn) = self.get_connection() {
            conn
        } else {
            return None;
        };
        let data = schema::ratio_table::table
            .find(osuname)
            .first::<Ratios>(&conn)
            .ok();
        match diesel::replace_into(schema::ratio_table::table)
            .values(&entry)
            .execute(&conn)
        {
            Ok(_) => debug!("Updated ratios of '{}'", osuname),
            Err(why) => warn!("Error while updating ratios: {}", why),
        }
        data
    }

    // -----------------------------
    // Table: bg_verified / map_tags
    // -----------------------------

    pub fn get_bg_verified(&self) -> DBResult<Vec<UserId>> {
        let conn = self.get_connection()?;
        let users = schema::bg_verified::table
            .load::<(u64,)>(&conn)?
            .into_iter()
            .map(|id| UserId(id.0))
            .collect();
        Ok(users)
    }

    pub fn add_tag_mapset(
        &self,
        mapset_id: u32,
        file_type: &str,
        gamemode: GameMode,
    ) -> DBResult<()> {
        use schema::map_tags::dsl::{beatmapset_id, filetype, mode};
        let conn = self.get_connection()?;
        diesel::insert_or_ignore_into(schema::map_tags::table)
            .values((
                beatmapset_id.eq(mapset_id),
                filetype.eq(file_type),
                mode.eq(gamemode as u8),
            ))
            .execute(&conn)?;
        Ok(())
    }

    pub fn set_tags_mapset(&self, mapset_id: u32, tag: MapsetTags, value: bool) -> DBResult<()> {
        use schema::map_tags::columns::beatmapset_id;
        let conn = self.get_connection()?;
        let entry = MapsetTagDB::with_value(mapset_id, tag, value);
        diesel::update(schema::map_tags::table.filter(beatmapset_id.eq(mapset_id)))
            .set(&entry)
            .execute(&conn)?;
        Ok(())
    }

    pub fn get_tags_mapset(&self, mapset_id: u32) -> DBResult<MapsetTagWrapper> {
        let conn = self.get_connection()?;
        let tags = schema::map_tags::table
            .find(mapset_id)
            .first::<MapsetTagDB>(&conn)?;
        Ok(tags.into())
    }

    pub fn get_all_tags_mapset(&self, gamemode: GameMode) -> DBResult<Vec<MapsetTagWrapper>> {
        use schema::map_tags::columns::mode;
        let conn = self.get_connection()?;
        let tags = schema::map_tags::table
            .filter(mode.eq(gamemode as u8))
            .load::<MapsetTagDB>(&conn)?;
        Ok(tags.into_iter().map(|tag| tag.into()).collect())
    }

    pub fn get_random_tags_mapset(&self, gamemode: GameMode) -> DBResult<MapsetTagWrapper> {
        use schema::map_tags::columns::mode;
        no_arg_sql_function!(RAND, (), "sql RAND()");
        let conn = self.get_connection()?;
        let tags = schema::map_tags::table
            .filter(mode.eq(gamemode as u8))
            .order(RAND)
            .first::<MapsetTagDB>(&conn)?;
        Ok(tags.into())
    }
}

fn mania_mod_bits(mods: GameMods) -> u32 {
    let valid = GameMods::DoubleTime | GameMods::Easy | GameMods::HalfTime | GameMods::NoFail;
    mods.bits() & valid.bits()
}

sql_function! {
    fn length(t: Text) -> Integer;
}

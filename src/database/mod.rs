mod models;

use models::BeatmapWrapper;
pub use models::{DBMapSet, MapsetTagWrapper, Platform, Ratios, StreamTrack, TwitchUser};

use crate::{commands::utility::MapsetTags, util::globals::AUTHORITY_ROLES, Guild};

use failure::Error;
use rosu::models::{Beatmap, GameMode, GameMods};
use serenity::model::id::{GuildId, UserId};
use sqlx::mysql::{MySql, MySqlPool};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};
use tokio::stream::StreamExt;

pub struct MySQL {
    pool: MySqlPool,
}

type DBResult<T> = Result<T, Error>;

impl MySQL {
    pub async fn new(database_url: &str) -> DBResult<Self> {
        let pool = MySqlPool::builder()
            .max_size(16)
            .build(database_url)
            .await?;
        Ok(Self { pool })
    }

    // ---------------------
    // Table: maps / mapsets
    // ---------------------

    pub async fn get_beatmap(&self, map_id: u32) -> DBResult<Beatmap> {
        let query = "SELECT * FROM \
                        (SELECT * FROM maps WHERE beatmap_id=?) as m \
                    JOIN mapsets as ms ON m.beatmapset_id=ms.beatmapset_id";
        let map: BeatmapWrapper = sqlx::query_as(query)
            .bind(map_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(map.into())
    }

    pub async fn get_beatmapset(&self, mapset_id: u32) -> DBResult<DBMapSet> {
        let mapset: DBMapSet = sqlx::query_as("SELECT * FROM mapsets WHERE beatmapset_id=?")
            .bind(mapset_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(mapset)
    }

    pub async fn get_beatmaps(&self, map_ids: &[u32]) -> DBResult<HashMap<u32, Beatmap>> {
        if map_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let subquery = String::from("SELECT * FROM maps WHERE beatmap_id IN").in_clause(map_ids);
        let query = format!(
            "SELECT * FROM ({}) as m JOIN mapsets as ms ON m.beatmapset_id=ms.beatmapset_id",
            subquery
        );
        let beatmaps = sqlx::query_as::<_, BeatmapWrapper>(&query)
            .fetch(&self.pool)
            .filter_map(|result| match result {
                Ok(map_wrapper) => {
                    let map: Beatmap = map_wrapper.into();
                    Some((map.beatmap_id, map))
                }
                Err(why) => {
                    warn!("Error while getting maps from DB: {}", why);
                    None
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect();
        Ok(beatmaps)
    }

    pub async fn insert_beatmap(&self, map: &Beatmap) -> DBResult<()> {
        // Important to do mapsets first for foreign key constrain
        _insert_beatmapset(&self.pool, map).await?;
        _insert_beatmap(&self.pool, map).await?;
        Ok(())
    }

    pub async fn insert_beatmaps(&self, maps: Vec<Beatmap>) -> DBResult<()> {
        if maps.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for map in maps {
            _insert_beatmapset(&mut tx, &map).await?;
            _insert_beatmap(&mut tx, &map).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // --------------------
    // Table: discord_users
    // --------------------

    pub async fn add_discord_link(&self, id: u64, name: &str) -> DBResult<()> {
        sqlx::query("INSERT INTO discord_users(discord_id, osu_name) VALUES (?,?)")
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_discord_link(&self, id: u64) -> Result<(), Error> {
        sqlx::query("DELETE FROM discord_users WHERE discord_id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_discord_links(&self) -> Result<HashMap<u64, String>, Error> {
        let links = sqlx::query_as("SELECT * FROM discord_users")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect();
        Ok(links)
    }

    // ----------------------------------
    // Table: pp_mania_mods / pp_ctb_mods
    // ----------------------------------

    pub async fn get_mod_pp(
        &self,
        map_id: u32,
        mode: GameMode,
        mut mods: GameMods,
    ) -> DBResult<Option<f32>> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("pp_mania_mods", mania_pp_mods_column(mods)?),
            GameMode::CTB => ("pp_ctb_mods", ctb_pp_mods_column(mods)?),
            _ => unreachable!(),
        };
        let query = format!("SELECT {} FROM {} WHERE beatmap_id=?", column, table);
        let pp: (Option<f32>,) = sqlx::query_as(&query)
            .bind(map_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(pp.0)
    }

    pub async fn insert_pp_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mut mods: GameMods,
        pp: f32,
    ) -> DBResult<()> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("pp_mania_mods", mania_pp_mods_column(mods)?),
            GameMode::CTB => ("pp_ctb_mods", ctb_pp_mods_column(mods)?),
            _ => unreachable!(),
        };
        let query = format!(
            "INSERT INTO {} (beatmap_id, {col}) VALUES ($1,$2) ON DUPLICATE KEY UPDATE {col}=$2",
            table,
            col = column
        );
        sqlx::query(&query)
            .bind(map_id)
            .bind(pp)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_pp_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mut mods: GameMods,
        pp: f32,
    ) -> DBResult<()> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("pp_mania_mods", mania_pp_mods_column(mods)?),
            GameMode::CTB => ("pp_ctb_mods", ctb_pp_mods_column(mods)?),
            _ => unreachable!(),
        };
        let query = format!("UPDATE {} SET {}=? WHERE beatmap_id=?", table, col = column);
        sqlx::query(&query)
            .bind(pp)
            .bind(map_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ----------------------------------------
    // Table: stars_mania_mods / stars_ctb_mods
    // ----------------------------------------

    pub async fn get_mod_stars(
        &self,
        map_id: u32,
        mode: GameMode,
        mut mods: GameMods,
    ) -> DBResult<Option<f32>> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("stars_mania_mods", mania_stars_mods_column(mods)?),
            GameMode::CTB => ("stars_ctb_mods", ctb_stars_mods_column(mods)?),
            _ => unreachable!(),
        };
        let query = format!("SELECT {} FROM {} WHERE beatmap_id=?", column, table);
        let stars: (Option<f32>,) = sqlx::query_as(&query)
            .bind(map_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(stars.0)
    }

    pub async fn insert_stars_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mut mods: GameMods,
        stars: f32,
    ) -> DBResult<()> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("stars_mania_mods", mania_stars_mods_column(mods)?),
            GameMode::CTB => ("stars_ctb_mods", ctb_stars_mods_column(mods)?),
            _ => unreachable!(),
        };
        let query = format!(
            "INSERT INTO {} (beatmap_id, {col}) VALUES ($1,$2) ON DUPLICATE KEY UPDATE {col}=$2",
            table,
            col = column
        );
        sqlx::query(&query)
            .bind(map_id)
            .bind(stars)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_stars_map(
        &self,
        map_id: u32,
        mode: GameMode,
        mut mods: GameMods,
        stars: f32,
    ) -> DBResult<()> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("stars_mania_mods", mania_stars_mods_column(mods)?),
            GameMode::CTB => ("stars_ctb_mods", ctb_stars_mods_column(mods)?),
            _ => unreachable!(),
        };
        let query = format!("UPDATE {} SET {}=? WHERE beatmap_id=?", table, col = column);
        sqlx::query(&query)
            .bind(stars)
            .bind(map_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------
    // Table: role_assign
    // ------------------

    pub async fn get_role_assigns(&self) -> DBResult<HashMap<(u64, u64), u64>> {
        let assigns = sqlx::query_as::<_, (u32, u64, u64, u64)>("SELECT * FROM role_assign")
            .fetch(&self.pool)
            .filter_map(|result| match result {
                Ok((_, c, m, r)) => Some(((c, m), r)),
                Err(why) => {
                    warn!("Error while getting roleassigns from DB: {}", why);
                    None
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect();
        Ok(assigns)
    }

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> DBResult<()> {
        sqlx::query("INSERT INTO role_assign(channel,message,role) VALUES (?,?,?)")
            .bind(channel)
            .bind(message)
            .bind(role)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -----------------------------------
    // Table: stream_tracks / twitch_users
    // -----------------------------------

    pub async fn add_twitch_user(&self, id: u64, name: &str) -> DBResult<()> {
        sqlx::query("INSERT INTO twitch_users(user_id,name) VALUES (?,?)")
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_stream_track(&self, channel: u64, user: u64, pf: Platform) -> DBResult<()> {
        sqlx::query("INSERT INTO stream_tracks(channel_id,user_id,platform) VALUES (?,?,?)")
            .bind(channel)
            .bind(user)
            .bind(pf as u8)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_twitch_users(&self) -> DBResult<HashMap<String, u64>> {
        let users = sqlx::query_as::<_, (u64, String)>("SELECT * FROM twitch_users")
            .fetch(&self.pool)
            .filter_map(|result| match result {
                Ok((id, name)) => Some((name, id)),
                Err(why) => {
                    warn!("Error while getting twitch users from DB: {}", why);
                    None
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect();
        Ok(users)
    }

    pub async fn get_stream_tracks(&self) -> DBResult<HashSet<StreamTrack>> {
        let tracks = sqlx::query_as::<_, StreamTrack>("SELECT * FROM stream_tracks")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect();
        Ok(tracks)
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64, pf: Platform) -> DBResult<()> {
        sqlx::query("DELETE FROM stream_tracks WHERE channel_id=? AND user_id=? AND platform=?")
            .bind(channel)
            .bind(user)
            .bind(pf as u8)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -------------
    // Table: guilds
    // -------------

    pub async fn get_guilds(&self) -> DBResult<HashMap<GuildId, Guild>> {
        let guilds = sqlx::query_as::<_, Guild>("SELECT * FROM guilds")
            .fetch(&self.pool)
            .filter_map(|result| match result {
                Ok(g) => Some((g.guild_id, g)),
                Err(why) => {
                    warn!("Error while getting guilds from DB: {}", why);
                    None
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect();
        Ok(guilds)
    }

    pub async fn insert_guild(&self, guild_id: u64) -> DBResult<Guild> {
        sqlx::query("INSERT INTO guilds(guild_id,with_lyrics,authorities) VALUES (?,?,?)")
            .bind(guild_id)
            .bind(true)
            .bind(AUTHORITY_ROLES)
            .execute(&self.pool)
            .await?;
        Ok(Guild::new(guild_id))
    }

    pub async fn update_guild_lyrics(&self, guild: u64, lyrics: bool) -> DBResult<()> {
        sqlx::query("UPDATE guilds SET with_lyrics=? WHERE guild_id=?")
            .bind(lyrics)
            .bind(guild)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_guild_authorities(&self, guild: u64, authorities: String) -> DBResult<()> {
        sqlx::query("UPDATE guilds SET authorities=? WHERE guild_id=?")
            .bind(authorities)
            .bind(guild)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -------------------
    // Table: bggame_stats
    // -------------------

    pub async fn increment_bggame_score(&self, user: u64) -> DBResult<()> {
        let query = "INSERT INTO bggame_stats(discord_id,score) VALUES (?,1) \
                    ON DUPLICATE KEY UPDATE score=score+1";
        sqlx::query(&query).bind(user).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_bggame_score(&self, user: u64) -> DBResult<u32> {
        let (_, score): (u64, u32) =
            sqlx::query_as("SELECT * FROM bggame_stats WHERE discord_id=?")
                .bind(user)
                .fetch_one(&self.pool)
                .await?;
        Ok(score)
    }

    pub async fn all_bggame_scores(&self) -> DBResult<Vec<(u64, u32)>> {
        let scores = sqlx::query_as("SELECT * FROM bggame_stats")
            .fetch_all(&self.pool)
            .await?;
        Ok(scores)
    }

    // ------------------
    // Table: ratio_table
    // ------------------

    pub async fn update_ratios(
        &self,
        name: &str,
        scores: &str,
        ratios: &str,
        misses: &str,
    ) -> DBResult<Option<Ratios>> {
        let old_ratios: Option<Ratios> = sqlx::query_as("SELECT * FROM ratio_table WHERE name=?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        sqlx::query("REPLACE INTO ratio_table(name,scores,ratios,misses) VALUES (?,?,?,?)")
            .bind(name)
            .bind(scores)
            .bind(ratios)
            .bind(misses)
            .execute(&self.pool)
            .await?;
        Ok(old_ratios)
    }

    // -----------------------------
    // Table: bg_verified / map_tags
    // -----------------------------

    pub async fn get_bg_verified(&self) -> DBResult<HashSet<UserId>> {
        let users = sqlx::query_as::<_, (u64,)>("SELECT * FROM bg_verified")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|(id,)| UserId(id))
            .collect();
        Ok(users)
    }

    pub async fn add_tag_mapset(
        &self,
        mapset_id: u32,
        filetype: &str,
        mode: GameMode,
    ) -> DBResult<()> {
        sqlx::query("INSERT IGNORE INTO map_tags(beatmapset_id, filetype, mode) VALUES (?,?,?)")
            .bind(mapset_id)
            .bind(filetype)
            .bind(mode as u8)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_tags_mapset(
        &self,
        mapset_id: u32,
        tags: MapsetTags,
        value: bool,
    ) -> DBResult<()> {
        let mut query = String::from("UPDATE map_tags SET").set_tags(",", tags, value)?;
        write!(query, " WHERE beatmapset_id={}", mapset_id)?;
        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_tags_mapset(&self, mapset_id: u32) -> DBResult<MapsetTagWrapper> {
        let tags = sqlx::query_as("SELECT * FROM map_tags WHERE beatmapset_id=?")
            .bind(mapset_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(tags)
    }

    pub async fn get_all_tags_mapset(&self, mode: GameMode) -> DBResult<Vec<MapsetTagWrapper>> {
        let tags = sqlx::query_as("SELECT * FROM map_tags WHERE mode=?")
            .bind(mode as u8)
            .fetch_all(&self.pool)
            .await?;
        Ok(tags)
    }

    pub async fn get_random_tags_mapset(&self, mode: GameMode) -> DBResult<MapsetTagWrapper> {
        let query = r#"SELECT
                        *
                    FROM
                        map_tags AS mt
                        JOIN (
                            SELECT
                                beatmapset_id
                            from
                                map_tags
                            WHERE
                                mode=?
                            ORDER BY
                                RAND()
                            LIMIT
                                1
                        ) as rndm ON mt.beatmapset_id = rndm.beatmapset_id"#;
        let tags = sqlx::query_as(&query)
            .bind(mode as u8)
            .fetch_one(&self.pool)
            .await?;
        Ok(tags)
    }

    pub async fn get_specific_tags_mapset(
        &self,
        mode: GameMode,
        included: MapsetTags,
        excluded: MapsetTags,
    ) -> DBResult<Vec<MapsetTagWrapper>> {
        if included.is_empty() && excluded.is_empty() {
            return self.get_all_tags_mapset(mode).await;
        }
        let mut query = format!("SELECT * FROM map_tags WHERE mode={}", mode as u8);
        query.push_str(" AND");
        if !included.is_empty() {
            query = query.set_tags(" AND ", included, true)?;
            if !excluded.is_empty() {
                query.push_str(" AND");
            }
        }
        if !excluded.is_empty() {
            query = query.set_tags(" AND ", excluded, false)?;
        }
        let mapsets = sqlx::query_as(&query).fetch_all(&self.pool).await?;
        Ok(mapsets)
    }
}

trait CustomSQL: Sized + std::fmt::Write {
    fn pop(&mut self) -> Option<char>;

    /// Adds (a,b,c,...) to self
    fn in_clause<I, T>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: std::fmt::Display,
    {
        let _ = write!(self, " (");
        for value in values {
            let _ = write!(self, "{},", value);
        }
        self.pop();
        let _ = write!(self, ")");
        self
    }

    /// Adds a delim b delim c delim... without whitespaces to self
    fn set_tags(mut self, delim: &str, tags: MapsetTags, value: bool) -> DBResult<Self> {
        let mut tags = tags.into_iter();
        let first_tag = match tags.next() {
            Some(first_tag) => first_tag,
            None => bail!("Cannot build update query without tags"),
        };
        let _ = write!(self, " {}={}", tag_column(first_tag), value as u8);
        for tag in tags {
            let _ = write!(self, "{}{}={}", delim, tag_column(tag), value as u8);
        }
        Ok(self)
    }
}

fn tag_column(tag: MapsetTags) -> &'static str {
    match tag {
        MapsetTags::Farm => "farm",
        MapsetTags::Streams => "streams",
        MapsetTags::Alternate => "alternate",
        MapsetTags::BlueSky => "bluesky",
        MapsetTags::Meme => "meme",
        MapsetTags::Old => "old",
        MapsetTags::Easy => "easy",
        MapsetTags::Hard => "hard",
        MapsetTags::Kpop => "kpop",
        MapsetTags::English => "english",
        MapsetTags::HardName => "hardname",
        MapsetTags::Weeb => "weeb",
        MapsetTags::Tech => "tech",
        _ => panic!("Only call tag_column with single tag argument"),
    }
}

impl CustomSQL for String {
    fn pop(&mut self) -> Option<char> {
        self.pop()
    }
}

fn ctb_pp_mods_column(mods: GameMods) -> DBResult<&'static str> {
    let valid = GameMods::Easy | GameMods::NoFail | GameMods::DoubleTime | GameMods::HalfTime;
    let m = match mods & valid {
        GameMods::NoMod => "NM",
        GameMods::Hidden => "HD",
        GameMods::HardRock => "HR",
        GameMods::DoubleTime => "DT",
        m if m == GameMods::Hidden | GameMods::HardRock => "HDHR",
        m if m == GameMods::Hidden | GameMods::DoubleTime => "HDDT",
        _ => bail!("No valid mod combination for ctb pp ({})", mods),
    };
    Ok(m)
}

fn mania_pp_mods_column(mods: GameMods) -> DBResult<&'static str> {
    let valid = GameMods::Easy | GameMods::NoFail | GameMods::DoubleTime | GameMods::HalfTime;
    let m = match mods & valid {
        GameMods::NoMod => "NM",
        GameMods::NoFail => "NF",
        GameMods::Easy => "EZ",
        GameMods::DoubleTime => "DT",
        GameMods::HalfTime => "HT",
        m if m == GameMods::NoFail | GameMods::Easy => "NFEZ",
        m if m == GameMods::NoFail | GameMods::DoubleTime => "NFDT",
        m if m == GameMods::Easy | GameMods::DoubleTime => "EZDT",
        m if m == GameMods::NoFail | GameMods::HalfTime => "NFHT",
        m if m == GameMods::Easy | GameMods::HalfTime => "EZHT",
        m if m == GameMods::NoFail | GameMods::Easy | GameMods::DoubleTime => "NFEZDT",
        m if m == GameMods::NoFail | GameMods::Easy | GameMods::HalfTime => "NFEZHT",
        _ => bail!("No valid mod combination for mania pp ({})", mods),
    };
    Ok(m)
}

fn ctb_stars_mods_column(mods: GameMods) -> DBResult<&'static str> {
    let valid = GameMods::Easy | GameMods::HardRock | GameMods::DoubleTime | GameMods::HalfTime;
    let m = match mods & valid {
        GameMods::Easy => "EZ",
        GameMods::HardRock => "HR",
        GameMods::DoubleTime => "DT",
        GameMods::HalfTime => "HT",
        m if m == GameMods::Easy | GameMods::DoubleTime => "EZDT",
        m if m == GameMods::HardRock | GameMods::DoubleTime => "HRDT",
        m if m == GameMods::Easy | GameMods::HalfTime => "EZHT",
        m if m == GameMods::HardRock | GameMods::HalfTime => "HRHT",
        _ => bail!("No valid mod combination for ctb stars ({})", mods),
    };
    Ok(m)
}

fn mania_stars_mods_column(mods: GameMods) -> DBResult<&'static str> {
    let valid = GameMods::DoubleTime | GameMods::HalfTime;
    let m = match mods & valid {
        GameMods::DoubleTime => "DT",
        GameMods::HalfTime => "HT",
        _ => bail!("No valid mod combination for mania stars ({})", mods),
    };
    Ok(m)
}

async fn _insert_beatmap<'c, E>(executor: E, map: &Beatmap) -> DBResult<()>
where
    E: sqlx::prelude::Executor<'c, Database = MySql>,
{
    let query = "INSERT IGNORE INTO maps (\
                    beatmap_id,\
                    beatmapset_id,\
                    mode,\
                    version,\
                    seconds_drain,\
                    seconds_total,\
                    bpm,\
                    stars,\
                    diff_cs,\
                    diff_od,\
                    diff_ar,\
                    diff_hp,\
                    count_circle,\
                    count_slider,\
                    count_spinner,\
                    max_combo\
                ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)";
    sqlx::query(query)
        .bind(map.beatmap_id)
        .bind(map.beatmapset_id)
        .bind(map.mode as u8)
        .bind(&map.version)
        .bind(map.seconds_drain)
        .bind(map.seconds_total)
        .bind(map.bpm)
        .bind(map.stars)
        .bind(map.diff_cs)
        .bind(map.diff_od)
        .bind(map.diff_ar)
        .bind(map.diff_hp)
        .bind(map.count_circle)
        .bind(map.count_slider)
        .bind(map.count_spinner)
        .bind(map.max_combo)
        .execute(executor)
        .await?;
    Ok(())
}

async fn _insert_beatmapset<'c, E>(executor: E, map: &Beatmap) -> DBResult<()>
where
    E: sqlx::prelude::Executor<'c, Database = MySql>,
{
    let query = "INSERT IGNORE INTO mapsets (\
                    beatmapset_id,\
                    artist,\
                    title,\
                    creator_id,\
                    creator,\
                    genre,\
                    language,\
                    approval_status,\
                    approved_date\
                ) VALUES (?,?,?,?,?,?,?,?,?)";
    sqlx::query(query)
        .bind(map.beatmapset_id)
        .bind(&map.artist)
        .bind(&map.title)
        .bind(map.creator_id)
        .bind(&map.creator)
        .bind(map.genre as u8)
        .bind(map.language as u8)
        .bind(map.approval_status as i8)
        .bind(map.approved_date)
        .execute(executor)
        .await?;
    Ok(())
}

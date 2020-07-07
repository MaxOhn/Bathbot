mod models;
mod util;

use models::BeatmapWrapper;
pub use models::{DBMapSet, GuildConfig, MapsetTagWrapper, Ratios, StreamTrack, TwitchUser};
use util::*;

use crate::{bail, util::bg_game::MapsetTags, BotResult};

use dashmap::{DashMap, ElementGuard};
use deadpool_postgres::{Manager, Pool};
use postgres_types::Type;
use rosu::models::{
    ApprovalStatus::{Approved, Loved, Ranked},
    Beatmap, GameMode, GameMods,
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};
use tokio::stream::StreamExt;
use tokio_postgres::{Config, NoTls};
use twilight::model::id::UserId;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!();
}

pub struct Database {
    pool: Pool,
}

impl Database {
    pub async fn new(database_url: &str) -> BotResult<Self> {
        let manager = Manager::new(Config::from_str(database_url)?, NoTls);
        let pool = Pool::new(manager, 10);
        let mut connection = pool.get().await?;

        embedded::migrations::runner()
            .run_async(&mut **connection)
            .await?;
        // .map_err(|e| Error::DatabaseMigration(e.to_string()))?;

        Ok(Self { pool })
    }

    // ---------------------
    // Table: maps / mapsets
    // ---------------------

    pub async fn get_beatmap(&self, map_id: u32) -> BotResult<Beatmap> {
        let query = r#"
SELECT
    *
FROM
    (
        SELECT
            *
        FROM
            maps
        WHERE
            beatmap_id = ?
    ) as m
    JOIN mapsets as ms ON m.beatmapset_id = ms.beatmapset_id"#;
        let map: BeatmapWrapper = sqlx::query_as(query)
            .bind(map_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(map.into())
    }

    pub async fn get_beatmapset(&self, mapset_id: u32) -> BotResult<Option<DBMapSet>> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "SELECT * FROM mapsets WHERE beatmapset_id=$1",
                &[Type::INT4],
            )
            .await?;
        client.query_one(&statement, &[&(mapset_id as i32)]).await
    }

    pub async fn get_beatmaps(&self, map_ids: &[u32]) -> BotResult<HashMap<u32, Beatmap>> {
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

    pub async fn insert_beatmap(&self, map: &Beatmap) -> BotResult<()> {
        match map.approval_status {
            Loved | Ranked | Approved => {
                // Important to do mapsets first for foreign key constrain
                _insert_beatmapset(&self.pool, map).await?;
                _insert_beatmap(&self.pool, map).await?;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn insert_beatmaps(&self, maps: &[Beatmap]) -> BotResult<()> {
        if maps.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for map in maps.iter() {
            match map.approval_status {
                Loved | Ranked | Approved => {
                    _insert_beatmapset(&mut tx, map).await?;
                    _insert_beatmap(&mut tx, map).await?;
                }
                _ => {}
            }
        }
        tx.commit().await?;
        Ok(())
    }

    // --------------------
    // Table: discord_users
    // --------------------

    pub async fn add_discord_link(&self, id: u64, name: &str) -> BotResult<()> {
        sqlx::query(
            r#"
INSERT INTO
    discord_users(discord_id, osu_name)
VALUES
    (?,?) ON DUPLICATE KEY
UPDATE
    osu_name=?"#,
        )
        .bind(id)
        .bind(name)
        .bind(name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_discord_link(&self, id: u64) -> BotResult<()> {
        sqlx::query("DELETE FROM discord_users WHERE discord_id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_discord_links(&self) -> BotResult<HashMap<u64, String>> {
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
    ) -> BotResult<Option<f32>> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("pp_mania_mods", mania_pp_mods_column(mods)?),
            GameMode::CTB => {
                let column = ctb_pp_mods_column(mods);
                if let Some(column) = column {
                    ("pp_ctb_mods", column)
                } else {
                    return Ok(None);
                }
            }
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
    ) -> BotResult<()> {
        if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("pp_mania_mods", mania_pp_mods_column(mods)?),
            GameMode::CTB => {
                let column = ctb_pp_mods_column(mods);
                if let Some(column) = column {
                    ("pp_ctb_mods", column)
                } else {
                    return Ok(());
                }
            }
            _ => unreachable!(),
        };
        let query = format!(
            r#"
INSERT INTO
    {} (beatmap_id, {col})
VALUES
    (?,?) ON DUPLICATE KEY
UPDATE
    {col}=?"#,
            table,
            col = column
        );
        sqlx::query(&query)
            .bind(map_id)
            .bind(pp)
            .bind(pp)
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
    ) -> BotResult<Option<f32>> {
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
    ) -> BotResult<()> {
        let mania_mods = GameMods::DoubleTime | GameMods::HalfTime;
        let ctb_mods =
            GameMods::Easy | GameMods::HardRock | GameMods::DoubleTime | GameMods::HalfTime;
        if (mode == GameMode::MNA && !mods.intersects(mania_mods))
            || (mode == GameMode::CTB && !mods.intersects(ctb_mods))
        {
            return Ok(());
        } else if mods.contains(GameMods::NightCore) {
            mods.remove(GameMods::NightCore);
            mods.insert(GameMods::DoubleTime);
        }
        let (table, column) = match mode {
            GameMode::MNA => ("stars_mania_mods", mania_stars_mods_column(mods)?),
            GameMode::CTB => ("stars_ctb_mods", ctb_stars_mods_column(mods)?),
            _ => unreachable!(),
        };
        let query = format!(
            r#"
INSERT INTO
    {} (beatmap_id, {col})
VALUES
    (?,?) ON DUPLICATE KEY
UPDATE
    {col}=?"#,
            table,
            col = column
        );
        sqlx::query(&query)
            .bind(map_id)
            .bind(stars)
            .bind(stars)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------
    // Table: role_assign
    // ------------------

    pub async fn get_role_assigns(&self) -> BotResult<HashMap<(u64, u64), u64>> {
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

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> BotResult<()> {
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

    pub async fn add_twitch_user(&self, id: u64, name: &str) -> BotResult<()> {
        sqlx::query("INSERT INTO twitch_users(user_id,name) VALUES (?,?)")
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        sqlx::query("INSERT INTO stream_tracks(channel_id,user_id) VALUES (?,?)")
            .bind(channel)
            .bind(user)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_twitch_users(&self) -> BotResult<HashMap<String, u64>> {
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

    pub async fn get_stream_tracks(&self) -> BotResult<HashSet<StreamTrack>> {
        let tracks = sqlx::query_as::<_, StreamTrack>("SELECT * FROM stream_tracks")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect();
        Ok(tracks)
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        sqlx::query(
            r#"
DELETE FROM
    stream_tracks
WHERE
    channel_id=?
    AND user_id=?"#,
        )
        .bind(channel)
        .bind(user)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -------------
    // Table: guilds
    // -------------

    pub async fn get_guild_config(&self, guild_id: u64) -> BotResult<GuildConfig> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed("SELECT config from guilds where id=$1", &[Type::INT8])
            .await?;
        if let Some(row) = client.query_one(&statement, &[&(guild_id as i64)]).await? {
            Ok(serde_json::from_value(row.get(0))?)
        } else {
            let config = GuildConfig::default();
            info!(
                "No config found for guild {}, inserting blank one",
                guild_id
            );
            let statement = client
                .prepare_typed(
                    "INSERT INTO guilds VALUES ($1, $2)",
                    &[Type::INT8, Type::JSON],
                )
                .await?;
            client
                .execute(
                    &statement,
                    &[
                        &(guild_id as i64),
                        &serde_json::to_value(&GuildConfig::default()).unwrap(),
                    ],
                )
                .await?;
            Ok(config)
        }
    }

    // pub async fn get_guilds(&self) -> BotResult<HashMap<GuildId, Guild>> {
    //     let guilds = sqlx::query_as::<_, Guild>("SELECT * FROM guilds")
    //         .fetch(&self.pool)
    //         .filter_map(|result| match result {
    //             Ok(g) => Some((g.guild_id, g)),
    //             Err(why) => {
    //                 warn!("Error while getting guilds from DB: {}", why);
    //                 None
    //             }
    //         })
    //         .collect::<Vec<_>>()
    //         .await
    //         .into_iter()
    //         .collect();
    //     Ok(guilds)
    // }

    pub async fn set_guild_config(&self, guild_id: u64, config: GuildConfig) -> BotResult<()> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "UPDATE guilds SET config=$1 WHERE id=$2",
                &[Type::JSON, Type::INT8],
            )
            .await?;
        client
            .execute(&statement, &[&config, &(guild_id as i64)])
            .await?;
        Ok(())
    }

    pub async fn insert_guild(&self, guild_id: u64) -> BotResult<GuildConfig> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "INSERT INTO guilds VALUES ($1,$2) ON CONFLICT DO NOTHING",
                &[Type::INT8, Type::JSON],
            )
            .await?;
        let config = GuildConfig::default();
        client
            .execute(&statement, &[&(guild_id as i64), &config])
            .await?;
        Ok(config)
    }

    // -------------------
    // Table: bggame_stats
    // -------------------

    pub async fn increment_bggame_score(&self, user_id: u64) -> BotResult<()> {
        let query = r#"
INSERT INTO
    bggame_stats
VALUES
    ($1,1)
ON CONFLICT DO
    UPDATE
        score = score + 1"#;
        let client = self.pool.get().await?;
        let statement = client.prepare_typed(query, &[Type::INT8]).await?;
        client.execute(&statement, &[&(user_id as i64)]).await?;
        Ok(())
    }

    pub async fn get_bggame_score(&self, user_id: u64) -> BotResult<u32> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "SELECT score FROM bggame_stats WHERE discord_id=$1",
                &[Type::INT8],
            )
            .await?;
        let score = client
            .query_one(&statement, &[&(user_id as i64)])
            .await?
            .map_or(0, |row| row.get(0));
        Ok(score)
    }

    pub async fn all_bggame_scores(&self) -> BotResult<Vec<(u64, u32)>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM bggame_stats").await?;
        let scores = client
            .query(&statement)
            .await?
            .into_iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect();
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
    ) -> BotResult<Option<Ratios>> {
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

    pub async fn get_bg_verified(&self) -> BotResult<HashSet<UserId>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT user_id FROM bg_verified").await?;
        let users = client
            .query(&statement)
            .await?
            .into_iter()
            .map(|row| UserId(row.get(0)))
            .collect();
        Ok(users)
    }

    pub async fn add_tag_mapset(
        &self,
        mapset_id: u32,
        filetype: &str,
        mode: GameMode,
    ) -> BotResult<()> {
        let query = "
INSERT
    INTO map_tags(beatmapset_id, filetype, mode)
VALUES
    ($1,$2,$3)";
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(query, &[Type::INT4, Type::BYTEA, Type::INT2])
            .await?;
        client
            .execute(&statement, &[&(mapset_id as i32), filetype, mode as i8])
            .await?;
        Ok(())
    }

    pub async fn set_tags_mapset(
        &self,
        mapset_id: u32,
        tags: MapsetTags,
        value: bool,
    ) -> BotResult<()> {
        let mut query = String::from("UPDATE map_tags SET").set_tags(",", tags, value)?;
        write!(query, " WHERE beatmapset_id={}", mapset_id)?;
        let client = self.pool.get().await?;
        let statement = client.prepare(&query).await?;
        client.execute(&statement).await?;
        Ok(())
    }

    // pub async fn get_tags_mapset(&self, mapset_id: u32) -> BotResult<MapsetTagWrapper> {
    //     let tags = sqlx::query_as("SELECT * FROM map_tags WHERE beatmapset_id=?")
    //         .bind(mapset_id)
    //         .fetch_one(&self.pool)
    //         .await?;
    //     Ok(tags)
    // }

    // pub async fn get_all_tags_mapset(&self, mode: GameMode) -> BotResult<Vec<MapsetTagWrapper>> {
    //     let tags = sqlx::query_as("SELECT * FROM map_tags WHERE mode=?")
    //         .bind(mode as u8)
    //         .fetch_all(&self.pool)
    //         .await?;
    //     Ok(tags)
    // }

    //     pub async fn get_random_tags_mapset(&self, mode: GameMode) -> BotResult<MapsetTagWrapper> {
    //         let query = r#"
    // SELECT
    //     *
    // FROM
    //     map_tags AS mt
    //     JOIN (
    //         SELECT
    //             beatmapset_id
    //         from
    //             map_tags
    //         WHERE
    //             mode=?
    //         ORDER BY
    //             RAND()
    //         LIMIT
    //             1
    //     ) as rndm ON mt.beatmapset_id = rndm.beatmapset_id"#;
    //         let tags = sqlx::query_as(&query)
    //             .bind(mode as u8)
    //             .fetch_one(&self.pool)
    //             .await?;
    //         Ok(tags)
    //     }

    // pub async fn get_specific_tags_mapset(
    //     &self,
    //     mode: GameMode,
    //     included: MapsetTags,
    //     excluded: MapsetTags,
    // ) -> BotResult<Vec<MapsetTagWrapper>> {
    //     if included.is_empty() && excluded.is_empty() {
    //         return self.get_all_tags_mapset(mode).await;
    //     }
    //     let mut query = format!("SELECT * FROM map_tags WHERE mode={}", mode as u8);
    //     query.push_str(" AND");
    //     if !included.is_empty() {
    //         query = query.set_tags(" AND ", included, true)?;
    //         if !excluded.is_empty() {
    //             query.push_str(" AND");
    //         }
    //     }
    //     if !excluded.is_empty() {
    //         query = query.set_tags(" AND ", excluded, false)?;
    //     }
    //     let mapsets = sqlx::query_as(&query).fetch_all(&self.pool).await?;
    //     Ok(mapsets)
    // }
}

fn ctb_pp_mods_column(mods: GameMods) -> Option<&'static str> {
    if (mods - GameMods::Perfect).is_empty() {
        return Some("NM");
    }
    let valid = GameMods::Hidden | GameMods::HardRock | GameMods::DoubleTime;
    let m = match mods & valid {
        GameMods::Hidden => "HD",
        GameMods::HardRock => "HR",
        GameMods::DoubleTime => "DT",
        m if m == GameMods::Hidden | GameMods::HardRock => "HDHR",
        m if m == GameMods::Hidden | GameMods::DoubleTime => "HDDT",
        _ => return None,
    };
    Some(m)
}

fn mania_pp_mods_column(mods: GameMods) -> BotResult<&'static str> {
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

fn ctb_stars_mods_column(mods: GameMods) -> BotResult<&'static str> {
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

fn mania_stars_mods_column(mods: GameMods) -> BotResult<&'static str> {
    let valid = GameMods::DoubleTime | GameMods::HalfTime;
    let m = match mods & valid {
        GameMods::DoubleTime => "DT",
        GameMods::HalfTime => "HT",
        _ => bail!("No valid mod combination for mania stars ({})", mods),
    };
    Ok(m)
}

async fn _insert_beatmap<'c, E>(executor: E, map: &Beatmap) -> BotResult<()>
where
    E: sqlx::prelude::Executor<'c, Database = MySql>,
{
    let query = r#"
INSERT IGNORE INTO maps (
    beatmap_id,
    beatmapset_id,
    mode,
    version,
    seconds_drain,
    seconds_total,
    bpm,
    stars,
    diff_cs,
    diff_od,
    diff_ar,
    diff_hp,
    count_circle,
    count_slider,
    count_spinner,
    max_combo
) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#;
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

async fn _insert_beatmapset<'c, E>(executor: E, map: &Beatmap) -> BotResult<()>
where
    E: sqlx::prelude::Executor<'c, Database = MySql>,
{
    let query = r#"
INSERT IGNORE INTO mapsets (
    beatmapset_id,
    artist,
    title,
    creator_id,
    creator,
    genre,
    language,
    approval_status,
    approved_date
) VALUES (?,?,?,?,?,?,?,?,?)"#;
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

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
use tokio_postgres::{Config, NoTls, Transaction};
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
        let query = "
SELECT
    *
FROM
    (
        SELECT
            *
        FROM
            maps
        WHERE
            beatmap_id=$1
    ) as m
    JOIN mapsets as ms ON m.beatmapset_id = ms.beatmapset_id
";
        let client = self.pool.get().await?;
        let statement = client.prepare_typed(query & [Type::INT4]).await?;
        let map = client.query_one(&statement, &[&(map_id as i32)]).await?;
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
        let client = self.pool.get().await?;
        let statement = client.prepare(query).await?;
        let maps = client
            .query(&statement)
            .await?
            .into_iter()
            .map(|row| {
                let map: Beatmap = row.into::<BeatmapWrapper>().into();
                (map.beatmap_id, map)
            })
            .collect();
        // let beatmaps = sqlx::query_as::<_, BeatmapWrapper>(&query)
        //     .fetch(&self.pool)
        //     .filter_map(|result| match result {
        //         Ok(map_wrapper) => {
        //             let map: Beatmap = map_wrapper.into();
        //             Some((map.beatmap_id, map))
        //         }
        //         Err(why) => {
        //             warn!("Error while getting maps from DB: {}", why);
        //             None
        //         }
        //     })
        //     .collect::<Vec<_>>()
        //     .await
        //     .into_iter()
        //     .collect();
        Ok(maps)
    }

    pub async fn insert_beatmap(&self, map: &Beatmap) -> BotResult<bool> {
        let client = self.pool.get().await?;
        let txn = client.transaction().await?;
        let result = _insert_beatmap(&txn, map).await?;
        txn.commit().await?;
        Ok(result)
    }

    async fn _insert_beatmap<'t, 'm>(
        txn: &'t Transaction<'t>,
        map: &'m Beatmap,
    ) -> BotResult<bool> {
        match map.approval_status {
            Loved | Ranked | Approved => {
                // Important to do mapsets first for foreign key constrain
                let mapset_query = format!(
                    "
INSERT INTO
    mapsets
VALUES
    ({},{},{},{},{},{},{},{},$1)
ON CONFLICT DO
    NOTHING
",
                    map.beatmapset_id,
                    map.artist,
                    map.title,
                    map.creator_id,
                    map.creator,
                    map.genre.to_string().to_lowercase(),
                    map.language.to_string().to_lowercase(),
                    map.approval_status.to_string().to_lowercase(),
                );
                let mapset_stmnt = txn.prepare_typed(&mapset_query, &[Type::DATE]);
                txn.execute(mapset_stmnt, &[&map.approved_date]).await?;

                let map_query = format!(
                    "
INSERT INTO
    maps
VALUES
    ({},{},{},{},{},{},{},{},{},{},{},{},{},{},$1)
ON CONFLICT DO 
    NOTHING
",
                    map.beatmap_id,
                    map.beatmapset_id,
                    match map.mode {
                        GameMode::STD => "osu",
                        GameMode::TKO => "taiko",
                        GameMode::CTB => "fruits",
                        GameMode::MNA => "mania",
                    },
                    map.version,
                    map.seconds_drain,
                    map.seconds_total,
                    map.bpm,
                    map.diff_cs,
                    map.diff_od,
                    map.diff_ar,
                    map.diff_hp,
                    map.count_circle,
                    map.count_slider,
                    map.count_spinner
                );
                let map_stmnt = txn.prepare_typed(map_query, &[Type::INT4]);
                txn.execute(map_stmnt, &[&map.max_combo]).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub async fn insert_beatmaps(&self, maps: &[Beatmap]) -> BotResult<usize> {
        if maps.is_empty() {
            return Ok(0);
        }
        let mut success = 0;
        let client = self.pool.get().await?;
        let txn = client.transaction().await?;
        for map in maps.iter() {
            if _insert_beatmap(&txn, map).await? {
                success += 1
            }
        }
        txn.commit().await?;
        Ok(success)
    }

    // --------------------
    // Table: discord_users
    // --------------------

    pub async fn add_discord_link(&self, user_id: u64, name: &str) -> BotResult<()> {
        let query = "
INSERT INTO
    discord_users
VALUES
    ($1,$2)
ON CONFLICT DO
    UPDATE
        SET osu_name=$2
";
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(query, &[Type::INT8, Type::BYTEA])
            .await?;
        client.execute(&statement, &[user_id as i64, name]).await?;
        Ok(())
    }

    pub async fn remove_discord_link(&self, user_id: u64) -> BotResult<()> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "DELETE FROM discord_users WHERE discord_id=$1",
                &[Type::INT8],
            )
            .await?;
        client.execute(statement, &[&user_id]).await?;
        Ok(())
    }

    pub async fn get_discord_links(&self) -> BotResult<HashMap<u64, String>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM discord_users").await?;
        let links = client
            .query(statement)
            .await?
            .into_iter()
            .map(|row| (row.get(0), row.get(1)))
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
            "
INSERT INTO
    {} (beatmap_id, {col})
VALUES
    (?,?) ON DUPLICATE KEY
UPDATE
    {col}=?
",
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
            "
INSERT INTO
    {} (beatmap_id, {col})
VALUES
    (?,?) ON DUPLICATE KEY
UPDATE
    {col}=?
",
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
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM role_assign").await?;
        let assigns = client
            .query(statement)
            .await?
            .into_iter()
            .map(|row| {
                let channel: i64 = row.get(0);
                let msg: i64 = row.get(1);
                let role: i64 = row.get(2);
                ((channel as u64, msg as u64), role as u64)
            })
            .collect();
        Ok(assigns)
    }

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> BotResult<()> {
        let query = "
INSERT INTO
    role_assign
VALUES
    ($1,$2,$3)
ON CONFLICT DO
    UPDATE
        SET role=$3
";
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(query, &[Type::INT8, Type::INT8, Type::INT8])
            .await?;
        client
            .execute(statement, &[channel as i64, message as i64, role as i64])
            .await?;
        Ok(())
    }

    // -----------------------------------
    // Table: stream_tracks / twitch_users
    // -----------------------------------

    pub async fn add_twitch_user(&self, user_id: u64, name: &str) -> BotResult<()> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "INSERT INTO twitch_users VALUES ($1,$2)",
                &[Type::INT8, Type::BYTEA],
            )
            .await?;
        client.execute(statement, &[user_id as i64, name]).await?;
        Ok(())
    }

    pub async fn add_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "INSERT INTO stream_tracks VALUES ($1,$2)",
                &[Type::INT8, Type::INT8],
            )
            .await?;
        client
            .execute(statement, &[channel as i64, user as i64])
            .await?;
        Ok(())
    }

    pub async fn get_twitch_users(&self, user_ids: &[u64]) -> BotResult<HashMap<String, u64>> {
        let query = String::from("SELECT * FROM twitch_users WHERE user_id IN").in_clause(user_ids);
        let client = self.pool.get().await?;
        let statement = client.prepare(&query).await?;
        let users = client
            .query(statement)
            .await?
            .into_iter()
            .map(|row| {
                let user_id: i64 = row.get(0);
                (row.get(1), user_id as u64)
            })
            .collect();
        Ok(users)
    }

    pub async fn get_stream_tracks(&self) -> BotResult<HashSet<(u64, u64)>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM stream_tracks").await?;
        let tracks = client
            .query(statement)
            .await?
            .into_iter()
            .map(|row| {
                let channel: i64 = row.get(0);
                let user: i64 = row.get(1);
                (channel as u64, user as u64)
            })
            .collect();
        Ok(tracks)
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        let client = self.pool.get().await?;
        let query = "
DELETE FROM
    stream_tracks
WHERE
    channel_id=$1
    AND user_id=$2
";
        let statement = client
            .prepare_typed(query, &[Type::INT8, Type::INT8])
            .await?;
        client
            .execute(statement, &[channel as i64, user as i64])
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
        let query = "
INSERT INTO
    bggame_stats
VALUES
    ($1,1)
ON CONFLICT DO
    UPDATE
        SET score=score+1
";
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

    // TODO: arguments
    pub async fn update_ratios(
        &self,
        name: &str,
        scores: &str,
        ratios: &str,
        misses: &str,
    ) -> BotResult<Option<Ratios>> {
        let select_query = "
SELECT 
    scores,ratios,misses
FROM
    ratio_table
WHERE
    name=$1
";
        let upsert_query = "
INSERT INTO
    ratio_table
VALUES
    ($1,$2,$3,$4)
ON CONFLICT DO
    UPDATE
        SET scores=$2,ratios=$3,misses=$4
";
        let client = self.pool.get().await?;
        let txn = client.transaction().await?;
        let select_stmnt = txn
            .prepare_typed(
                select_query,
                &[
                    Type::BYTEA,
                    Type::INT2_ARRAY,
                    Type::FLOAT4_ARRAY,
                    Type::FLOAT4_ARRAY,
                ],
            )
            .await?;
        let row = txn
            .query_opt(select_stmnt, &[name, scores, ratios, misses])
            .await?;
        let upsert_stmnt = txn
            .prepare_typed(
                upsert_query,
                &[
                    Type::BYTEA,
                    Type::INT2_ARRAY,
                    Type::FLOAT4_ARRAY,
                    Type::FLOAT4_ARRAY,
                ],
            )
            .await?;
        txn.execute(upsert_stmnt, &[name, scores, ratios, misses])
            .await?;
        txn.commit().await?;
        let old_ratios = row.map(|row| Ratios {
            scores: row.get(0),
            ratios: row.get(1),
            misses: row.get(2),
        });
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
    INTO map_tags
VALUES
    ($1,$2,$3)
";
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

    pub async fn get_tags_mapset(&self, mapset_id: u32) -> BotResult<MapsetTagWrapper> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "SELECT * FROM map_tags WHERE beatmapset_id=$1",
                &[Type::INT4],
            )
            .await?;
        let tags = client
            .query_one(statement, &[mapset_id as i64])
            .await?
            .into();
        Ok(tags)
    }

    pub async fn get_all_tags_mapset(&self, mode: GameMode) -> BotResult<Vec<MapsetTagWrapper>> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed("SELECT * FROM map_tags WHERE mode=$1", &[Type::BYTEA])
            .await?;
        let mode = match mode {
            GameMode::STD => "osu",
            GameMode::TKO => "taiko",
            GameMode::CTB => "fuits",
            GameMode::MNA => "mania",
        };
        let tags = client
            .query(statement, &[mode])
            .await?
            .into_iter()
            .map(|row| row.into())
            .collect();
        Ok(tags)
    }

    pub async fn get_random_tags_mapset(&self, mode: GameMode) -> BotResult<MapsetTagWrapper> {
        let query = "
SELECT
    *
FROM
    map_tags AS mt
    JOIN (
        SELECT
            beatmapset_id
        FROM
            map_tags
        WHERE
            mode=?
        ORDER BY
            RAND()
        LIMIT
            1
    ) as rndm ON mt.beatmapset_id = rndm.beatmapset_id
";
    }

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

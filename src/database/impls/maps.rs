use crate::{
    database::{util::CustomSQL, BeatmapWrapper, DBMapSet},
    BotResult, Database,
};

use rosu::models::{
    ApprovalStatus::{Approved, Loved, Ranked},
    Beatmap,
};
use sqlx::PgConnection;
use std::collections::HashMap;
use tokio::stream::StreamExt;

impl Database {
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
            beatmap_id = ?
    ) as m
    JOIN mapsets as ms ON m.beatmapset_id = ms.beatmapset_id
    ";
        let map: BeatmapWrapper = sqlx::query_as(query)
            .bind(map_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(map.into())
    }

    pub async fn get_beatmapset(&self, mapset_id: u32) -> BotResult<DBMapSet> {
        let mapset: DBMapSet = sqlx::query_as("SELECT * FROM mapsets WHERE beatmapset_id=?")
            .bind(mapset_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(mapset)
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

    pub async fn insert_beatmap(&self, map: &Beatmap) -> BotResult<bool> {
        let mut txn = self.pool.begin().await?;
        let result = _insert_map(&mut *txn, map).await?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn insert_beatmaps(&self, maps: &[Beatmap]) -> BotResult<usize> {
        if maps.is_empty() {
            return Ok(0);
        }
        let mut success = 0;
        let mut txn = self.pool.begin().await?;
        for map in maps.iter() {
            if _insert_map(&mut *txn, map).await? {
                success += 1
            }
        }
        txn.commit().await?;
        Ok(success)
    }
}

async fn _insert_map(conn: &mut PgConnection, map: &Beatmap) -> BotResult<bool> {
    match map.approval_status {
        Loved | Ranked | Approved => {
            // Crucial to do mapsets first for foreign key constrain
            _insert_beatmapset(conn, map).await?;
            _insert_beatmap(conn, map).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn _insert_beatmapset(conn: &mut PgConnection, map: &Beatmap) -> BotResult<()> {
    let mapset_query = format!(
        "
INSERT INTO
    mapsets
VALUES
    ({},$1,$2,{},$3,{},{},{},$4)
ON CONFLICT (beatmapset_id) DO
    NOTHING
",
        map.beatmapset_id,
        map.creator_id,
        map.genre as u8,
        map.language as u8,
        map.approval_status as i8,
    );
    sqlx::query(&mapset_query)
        .bind(&map.artist)
        .bind(&map.title)
        .bind(&map.creator)
        .bind(map.approved_date)
        .execute(conn)
        .await?;
    Ok(())
}

async fn _insert_beatmap(conn: &mut PgConnection, map: &Beatmap) -> BotResult<()> {
    let map_query = format!(
        "
    INSERT INTO
        maps
    VALUES
        ({},{},{},$1,{},{},{},{},{},{},{},{},{},{},$2)
    ON CONFLICT (beatmap_id) DO
        NOTHING
    ",
        map.beatmap_id,
        map.beatmapset_id,
        map.mode as u8,
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
    sqlx::query(&map_query)
        .bind(&map.version)
        .bind(&map.max_combo)
        .execute(conn)
        .await?;
    Ok(())
}

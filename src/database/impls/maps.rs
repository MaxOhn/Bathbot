use crate::{
    database::{BeatmapWrapper, DBMapSet},
    BotResult, Database,
};

use postgres_types::Type;
use rosu::models::{
    ApprovalStatus::{Approved, Loved, Ranked},
    Beatmap,
};
use std::collections::HashMap;
use tokio_postgres::Transaction;

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
            beatmap_id=$1
    ) as m
    JOIN mapsets as ms ON m.beatmapset_id = ms.beatmapset_id
";
        let client = self.pool.get().await?;
        let statement = client.prepare_typed(query, &[Type::INT4]).await?;
        let row = client.query_one(&statement, &[&(map_id as i32)]).await?;
        Ok(BeatmapWrapper::from(row).into())
    }

    // pub async fn get_beatmapset(&self, mapset_id: u32) -> BotResult<Option<DBMapSet>> {
    //     let client = self.pool.get().await?;
    //     let statement = client
    //         .prepare_typed(
    //             "SELECT * FROM mapsets WHERE beatmapset_id=$1",
    //             &[Type::INT4],
    //         )
    //         .await?;
    //     client.query_one(&statement, &[&(mapset_id as i32)]).await
    // }

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
        let statement = client.prepare(&query).await?;
        let maps = client
            .query(&statement, &[])
            .await?
            .into_iter()
            .map(|row| {
                let map: Beatmap = BeatmapWrapper::from(row).into();
                (map.beatmap_id, map)
            })
            .collect();
        Ok(maps)
    }

    pub async fn insert_beatmap(&self, map: &Beatmap) -> BotResult<bool> {
        let mut client = self.pool.get().await?;
        let txn = client.transaction().await?;
        let result = _insert_beatmap(&txn, map).await?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn insert_beatmaps(&self, maps: &[Beatmap]) -> BotResult<usize> {
        if maps.is_empty() {
            return Ok(0);
        }
        let mut success = 0;
        let mut client = self.pool.get().await?;
        let txn = client.transaction().await?;
        for map in maps.iter() {
            if _insert_beatmap(&txn, map).await? {
                success += 1
            }
        }
        txn.commit().await?;
        Ok(success)
    }
}

async fn _insert_beatmap<'t, 'm>(txn: &'t Transaction<'t>, map: &'m Beatmap) -> BotResult<bool> {
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
            let mapset_stmnt = txn.prepare_typed(&mapset_query, &[Type::DATE]).await?;
            txn.execute(&mapset_stmnt, &[&(map.approved_date)]).await?;

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
                mode_enum(map.mode),
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
            let map_stmnt = txn.prepare_typed(&map_query, &[Type::INT4]).await?;
            txn.execute(&map_stmnt, &[&map.max_combo]).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

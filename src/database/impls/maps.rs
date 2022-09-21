use eyre::{Result, WrapErr};
use futures::{
    future::{BoxFuture, FutureExt},
    stream::{StreamExt, TryStreamExt},
};
use hashbrown::HashMap;
use rosu_v2::prelude::{
    Beatmap, Beatmapset, GameMode,
    RankStatus::{Approved, Loved, Ranked},
};
use sqlx::PgConnection;

use crate::{
    database::{DBBeatmap, DBBeatmapset},
    util::hasher::IntHasher,
    Database,
};

macro_rules! invalid_status {
    ($obj:ident) => {
        !matches!($obj.status, Ranked | Loved | Approved)
    };
}

fn should_not_be_stored(map: &Beatmap) -> bool {
    invalid_status!(map) || map.convert || (map.mode != GameMode::Mania && map.max_combo.is_none())
}

impl Database {
    pub async fn get_beatmap(&self, map_id: u32, with_mapset: bool) -> Result<Beatmap> {
        let mut conn = self.pool.acquire().await?;

        let query = sqlx::query_as!(
            DBBeatmap,
            "SELECT * FROM maps WHERE map_id=$1",
            map_id as i32
        );

        let row = query
            .fetch_one(&mut conn)
            .await
            .wrap_err("failed to get map")?;

        let mut map = Beatmap::from(row);

        if with_mapset {
            let query = sqlx::query_as!(
                DBBeatmapset,
                "SELECT * FROM mapsets WHERE mapset_id=$1",
                map.mapset_id as i32
            );

            let mapset = query
                .fetch_one(&mut conn)
                .await
                .wrap_err("failed to get mapset")?;

            map.mapset.replace(mapset.into());
        }

        Ok(map)
    }

    pub async fn get_beatmapset<T: From<DBBeatmapset>>(&self, mapset_id: u32) -> Result<T> {
        let query = sqlx::query_as!(
            DBBeatmapset,
            "SELECT * FROM mapsets WHERE mapset_id=$1",
            mapset_id as i32
        );

        let row = query.fetch_one(&self.pool).await?;

        Ok(row.into())
    }

    pub async fn get_beatmap_combo(&self, map_id: u32) -> Result<Option<u32>> {
        let row = sqlx::query!("SELECT max_combo FROM maps WHERE map_id=$1", map_id as i32)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.max_combo.map(|c| c as u32))
    }

    pub async fn get_beatmaps_combo(
        &self,
        map_ids: &[i32],
    ) -> Result<HashMap<u32, Option<u32>, IntHasher>> {
        let mut combos = HashMap::with_capacity_and_hasher(map_ids.len(), IntHasher);

        let query = sqlx::query!(
            "SELECT map_id,max_combo FROM maps WHERE map_id=ANY($1)",
            map_ids
        );

        let mut rows = query.fetch(&self.pool);

        while let Some(row) = rows.next().await.transpose()? {
            combos.insert(row.map_id as u32, row.max_combo.map(|c| c as u32));
        }

        Ok(combos)
    }

    pub async fn get_beatmaps(
        &self,
        map_ids: &[i32],
        with_mapset: bool,
    ) -> Result<HashMap<u32, Beatmap, IntHasher>> {
        if map_ids.is_empty() {
            return Ok(HashMap::default());
        }

        let mut conn = self
            .pool
            .acquire()
            .await
            .wrap_err("failed to acquire connection")?;

        let query = sqlx::query_as!(
            DBBeatmap,
            "SELECT * FROM maps WHERE map_id=ANY($1)",
            map_ids
        );

        let mut stream = query
            .fetch(&mut conn)
            .map_ok(Beatmap::from)
            .map_ok(|m| (m.map_id, m));

        let mut beatmaps = HashMap::with_capacity_and_hasher(map_ids.len(), IntHasher);

        while let Some((id, mut map)) = stream.next().await.transpose()? {
            if with_mapset {
                let query = sqlx::query_as!(
                    DBBeatmapset,
                    "SELECT * FROM mapsets WHERE mapset_id=$1",
                    map.mapset_id as i32
                );

                let mapset = query.fetch_one(&self.pool).await?;
                map.mapset.replace(mapset.into());
            }

            beatmaps.insert(id, map);
        }

        Ok(beatmaps)
    }

    pub async fn insert_beatmapset(&self, mapset: &Beatmapset) -> Result<bool> {
        if invalid_status!(mapset) {
            return Ok(false);
        }

        let mut conn = self
            .pool
            .acquire()
            .await
            .wrap_err("failed to acquire connection")?;

        insert_mapset_(&mut conn, mapset).await.map(|_| true)
    }

    pub async fn insert_beatmap(&self, map: &Beatmap) -> Result<bool> {
        if should_not_be_stored(map) {
            return Ok(false);
        }

        let mut conn = self
            .pool
            .acquire()
            .await
            .wrap_err("failed to acquire connection")?;

        insert_map_(&mut conn, map).await.map(|_| true)
    }

    pub async fn insert_beatmaps(&self, maps: impl Iterator<Item = &Beatmap>) -> Result<usize> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .wrap_err("failed to acquire connection")?;

        let mut count = 0;

        for map in maps {
            if should_not_be_stored(map) {
                continue;
            }

            insert_map_(&mut conn, map).await?;
            count += 1;
        }

        Ok(count)
    }
}

async fn insert_map_(conn: &mut PgConnection, map: &Beatmap) -> Result<()> {
    let max_combo = if map.mode == GameMode::Mania {
        None
    } else if let Some(combo) = map.max_combo {
        Some(combo as i32)
    } else {
        bail!("cannot insert {:?} map without combo", map.mode);
    };

    let query = sqlx::query!(
        "INSERT INTO maps (\
            map_id,\
            mapset_id,\
            checksum,\
            version,\
            seconds_total,\
            seconds_drain,\
            count_circles,\
            count_sliders,\
            count_spinners,\
            hp,\
            cs,\
            od,\
            ar,\
            mode,\
            status,\
            last_update,\
            stars,\
            bpm,\
            max_combo,\
            user_id\
        )\
        VALUES\
        ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)\
        ON CONFLICT (map_id) DO NOTHING",
        map.map_id as i32,
        map.mapset_id as i32,
        map.checksum,
        map.version,
        map.seconds_total as i32,
        map.seconds_drain as i32,
        map.count_circles as i32,
        map.count_sliders as i32,
        map.count_spinners as i32,
        map.hp,
        map.cs,
        map.od,
        map.ar,
        map.mode as i16,
        map.status as i16,
        map.last_updated,
        map.stars,
        map.bpm,
        max_combo,
        map.creator_id as i32,
    );

    query
        .execute(&mut *conn)
        .await
        .wrap_err("failed to insert map")?;

    if let Some(ref mapset) = map.mapset {
        insert_mapset_(conn, mapset)
            .await
            .wrap_err("failed to insert mapset")?;
    }

    Ok(())
}

fn insert_mapset_<'a>(
    conn: &'a mut PgConnection,
    mapset: &'a Beatmapset,
) -> BoxFuture<'a, Result<()>> {
    let fut = async move {
        let query = sqlx::query!(
            "INSERT INTO mapsets (\
                mapset_id,\
                user_id,\
                artist,\
                title,\
                creator,\
                status,\
                ranked_date,\
                bpm\
            )\
            VALUES\
            ($1,$2,$3,$4,$5,$6,$7,$8)\
            ON CONFLICT (mapset_id) DO NOTHING",
            mapset.mapset_id as i32,
            mapset.creator_id as i32,
            mapset.artist,
            mapset.title,
            mapset.creator_name.as_str(),
            mapset.status as i16,
            mapset.ranked_date,
            mapset.bpm,
        );

        query
            .execute(&mut *conn)
            .await
            .wrap_err("failed to insert mapset")?;

        if let Some(ref maps) = mapset.maps {
            for map in maps {
                insert_map_(conn, map)
                    .await
                    .wrap_err("failed to insert map")?;
            }
        }

        Ok(())
    };

    fut.boxed()
}

use std::{collections::HashMap, hash::BuildHasher};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use rosu_v2::prelude::{GameMode, Score, ScoreStatistics};
use sqlx::{pool::PoolConnection, Executor, Postgres};

use crate::{
    database::Database,
    model::osu::{
        DbScore, DbScoreAny, DbScoreBeatmapRaw, DbScoreBeatmapsetRaw, DbScoreCatch, DbScoreMania,
        DbScoreOsu, DbScoreTaiko, DbScoreUserRaw, DbScores,
    },
};

macro_rules! select_scores {
    ( $fn:ident, $ty:ident, $mode:ident: $query:literal ) => {
        async fn $fn(
            conn: &mut PoolConnection<Postgres>,
            user_ids: &[i32],
            country_code: Option<&str>,
            map_id: Option<i32>,
            mods_include: Option<i32>,
            mods_exclude: Option<i32>,
            mods_exact: Option<i32>,
        ) -> Result<Vec<DbScore>> {
            let query = sqlx::query_as!(
                $ty,
                $query,
                user_ids,
                country_code,
                mods_include,
                mods_exclude,
                mods_exact,
                map_id,
            );

            let mut scores = Vec::new();
            let mut rows = query.fetch(conn);

            while let Some(row_res) = rows.next().await {
                let row = row_res.wrap_err("Failed to fetch next score")?;
                scores.push(DbScore::from((row, GameMode::$mode)));
            }

            Ok(scores)
        }
    };
}

impl Database {
    select_scores!(select_osu_scores, DbScoreOsu, Osu:
r#"WITH scores AS (
  SELECT 
    score_id, 
    user_id, 
    map_id, 
    mods, 
    score, 
    maxcombo, 
    grade, 
    count50, 
    count100, 
    count300, 
    countmiss, 
    ended_at 
  FROM 
    osu_scores 
  WHERE 
    gamemode = 0 
    AND user_id = ANY($1)
    AND (
      -- map id
      $6 :: INT4 IS NULL 
      OR map_id = $6
    ) 
    AND (
      -- country code
      $2 :: VARCHAR(2) IS NULL 
      OR (
        SELECT 
          country_code 
        FROM 
          osu_user_stats 
        WHERE 
          user_id = osu_scores.user_id
      ) = $2
    ) 
    AND (
      -- include mods
      $3 :: INT4 IS NULL 
      OR (
        $3 != 0 
        AND $3 :: bit(32) & mods :: bit(32) = $3 :: bit(32)
      ) 
      OR (
        $3 = 0 
        AND mods = 0
      )
    ) 
    AND (
      -- exclude mods
      $4 :: INT4 IS NULL 
      OR (
        $4 != 0 
        AND $4 :: bit(32) & mods :: bit(32) != $4 :: bit(32)
      ) 
      OR (
        $4 = 0 
        AND mods > 0
      )
    ) 
    AND (
      -- exact mods
      $5 :: INT4 IS NULL 
      OR mods = $5
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  maxcombo, 
  grade, 
  count50, 
  count100, 
  count300, 
  countmiss, 
  ended_at, 
  pp :: FLOAT4, 
  stars :: FLOAT4 
FROM 
  scores 
  LEFT JOIN osu_scores_performance AS pp ON scores.score_id = pp.score_id 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty
  ) AS stars ON scores.map_id = stars.map_id 
  AND scores.mods = stars.mods 
ORDER BY 
  user_id, 
  scores.map_id, 
  scores.mods, 
  ended_at DESC"#
    );

    select_scores!(select_taiko_scores, DbScoreTaiko, Taiko:
r#"WITH scores AS (
  SELECT 
    score_id, 
    user_id, 
    map_id, 
    mods, 
    score, 
    maxcombo, 
    grade, 
    count100, 
    count300, 
    countmiss, 
    ended_at 
  FROM 
    osu_scores 
  WHERE 
    gamemode = 1 
    AND user_id = ANY($1) 
    AND (
      -- map id
      $6 :: INT4 IS NULL 
      OR map_id = $6
    ) 
    AND (
      -- country code
      $2 :: VARCHAR(2) IS NULL 
      OR (
        SELECT 
          country_code 
        FROM 
          osu_user_stats 
        WHERE 
          user_id = osu_scores.user_id
      ) = $2
    ) 
    AND (
      -- include mods
      $3 :: INT4 IS NULL 
      OR (
        $3 != 0 
        AND $3 :: bit(32) & mods :: bit(32) = $3 :: bit(32)
      ) 
      OR (
        $3 = 0 
        AND mods = 0
      )
    ) 
    AND (
      -- exclude mods
      $4 :: INT4 IS NULL 
      OR (
        $4 != 0 
        AND $4 :: bit(32) & mods :: bit(32) != $4 :: bit(32)
      ) 
      OR (
        $4 = 0 
        AND mods > 0
      )
    ) 
    AND (
      -- exact mods
      $5 :: INT4 IS NULL 
      OR mods = $5
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  maxcombo, 
  grade, 
  count100, 
  count300, 
  countmiss, 
  ended_at, 
  pp :: FLOAT4, 
  stars :: FLOAT4 
FROM 
  scores 
  LEFT JOIN osu_scores_performance AS pp ON scores.score_id = pp.score_id 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty_taiko
  ) AS stars ON scores.map_id = stars.map_id 
  AND scores.mods = stars.mods 
ORDER BY 
  user_id, 
  scores.map_id, 
  scores.mods, 
  ended_at DESC"#
    );

    select_scores!(select_catch_scores, DbScoreCatch, Catch:
r#"WITH scores AS (
  SELECT 
    score_id, 
    user_id, 
    map_id, 
    mods, 
    score, 
    maxcombo, 
    grade, 
    count50, 
    count100, 
    count300, 
    countkatu, 
    countmiss, 
    ended_at 
  FROM 
    osu_scores 
  WHERE 
    gamemode = 2 
    AND user_id = ANY($1) 
    AND (
      -- map id
      $6 :: INT4 IS NULL 
      OR map_id = $6
    ) 
    AND (
      -- country code
      $2 :: VARCHAR(2) IS NULL 
      OR (
        SELECT 
          country_code 
        FROM 
          osu_user_stats 
        WHERE 
          user_id = osu_scores.user_id
      ) = $2
    ) 
    AND (
      -- include mods
      $3 :: INT4 IS NULL 
      OR (
        $3 != 0 
        AND $3 :: bit(32) & mods :: bit(32) = $3 :: bit(32)
      ) 
      OR (
        $3 = 0 
        AND mods = 0
      )
    ) 
    AND (
      -- exclude mods
      $4 :: INT4 IS NULL 
      OR (
        $4 != 0 
        AND $4 :: bit(32) & mods :: bit(32) != $4 :: bit(32)
      ) 
      OR (
        $4 = 0 
        AND mods > 0
      )
    ) 
    AND (
      -- exact mods
      $5 :: INT4 IS NULL 
      OR mods = $5
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  maxcombo, 
  grade, 
  count50, 
  count100, 
  count300, 
  countkatu, 
  countmiss, 
  ended_at, 
  pp :: FLOAT4, 
  stars :: FLOAT4 
FROM 
  scores 
  LEFT JOIN osu_scores_performance AS pp ON scores.score_id = pp.score_id 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty_catch
  ) AS stars ON scores.map_id = stars.map_id 
  AND scores.mods = stars.mods 
ORDER BY 
  user_id, 
  scores.map_id, 
  scores.mods, 
  ended_at DESC"#
    );

    select_scores!(select_mania_scores, DbScoreMania, Mania:
r#"WITH scores AS (
  SELECT 
    score_id, 
    user_id, 
    map_id, 
    mods, 
    score, 
    maxcombo, 
    grade, 
    count50, 
    count100, 
    count300, 
    countgeki, 
    countkatu, 
    countmiss, 
    ended_at 
  FROM 
    osu_scores 
  WHERE 
    gamemode = 3 
    AND user_id = ANY($1) 
    AND (
      -- map id
      $6 :: INT4 IS NULL 
      OR map_id = $6
    ) 
    AND (
      -- country code
      $2 :: VARCHAR(2) IS NULL 
      OR (
        SELECT 
          country_code 
        FROM 
          osu_user_stats 
        WHERE 
          user_id = osu_scores.user_id
      ) = $2
    ) 
    AND (
      -- include mods
      $3 :: INT4 IS NULL 
      OR (
        $3 != 0 
        AND $3 :: bit(32) & mods :: bit(32) = $3 :: bit(32)
      ) 
      OR (
        $3 = 0 
        AND mods = 0
      )
    ) 
    AND (
      -- exclude mods
      $4 :: INT4 IS NULL 
      OR (
        $4 != 0 
        AND $4 :: bit(32) & mods :: bit(32) != $4 :: bit(32)
      ) 
      OR (
        $4 = 0 
        AND mods > 0
      )
    ) 
    AND (
      -- exact mods
      $5 :: INT4 IS NULL 
      OR mods = $5
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  maxcombo, 
  grade, 
  count50, 
  count100, 
  count300, 
  countgeki, 
  countkatu, 
  countmiss, 
  ended_at, 
  pp :: FLOAT4, 
  stars :: FLOAT4 
FROM 
  scores 
  LEFT JOIN osu_scores_performance AS pp ON scores.score_id = pp.score_id 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty_mania
  ) AS stars ON scores.map_id = stars.map_id 
  AND scores.mods = stars.mods 
ORDER BY 
  user_id, 
  scores.map_id, 
  scores.mods, 
  ended_at DESC"#
    );

    async fn select_any_scores(
        conn: &mut PoolConnection<Postgres>,
        user_ids: &[i32],
        country_code: Option<&str>,
        map_id: Option<i32>,
        mods_include: Option<i32>,
        mods_exclude: Option<i32>,
        mods_exact: Option<i32>,
    ) -> Result<Vec<DbScore>> {
        let query = sqlx::query_as!(
            DbScoreAny,
            r#"
WITH scores AS (
  SELECT 
    score_id, 
    user_id, 
    map_id, 
    gamemode, 
    mods, 
    score, 
    maxcombo, 
    grade, 
    count50, 
    count100, 
    count300, 
    countgeki, 
    countkatu, 
    countmiss, 
    ended_at 
  FROM 
    osu_scores 
  WHERE 
    user_id = ANY($1) 
    AND (
      -- map id
      $6 :: INT4 IS NULL 
      OR map_id = $6
    ) 
    AND (
      -- country code
      $2 :: VARCHAR(2) IS NULL 
      OR (
        SELECT 
          country_code 
        FROM 
          osu_user_stats 
        WHERE 
          user_id = osu_scores.user_id
      ) = $2
    ) 
    AND (
      -- include mods
      $3 :: INT4 IS NULL 
      OR (
        $3 != 0 
        AND $3 :: bit(32) & mods :: bit(32) = $3 :: bit(32)
      ) 
      OR (
        $3 = 0 
        AND mods = 0
      )
    ) 
    AND (
      -- exclude mods
      $4 :: INT4 IS NULL 
      OR (
        $4 != 0 
        AND $4 :: bit(32) & mods :: bit(32) != $4 :: bit(32)
      ) 
      OR (
        $4 = 0 
        AND mods > 0
      )
    ) 
    AND (
      -- exact mods
      $5 :: INT4 IS NULL 
      OR mods = $5
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, gamemode, 
    scores.mods
  ) user_id, 
  scores.map_id, 
  gamemode, 
  scores.mods, 
  score, 
  maxcombo, 
  grade, 
  count50, 
  count100, 
  count300, 
  countgeki, 
  countkatu, 
  countmiss, 
  ended_at, 
  pp :: FLOAT4, 
  stars_osu.stars :: FLOAT4 AS stars_osu, 
  stars_taiko.stars :: FLOAT4 AS stars_taiko, 
  stars_catch.stars :: FLOAT4 AS stars_catch, 
  stars_mania.stars :: FLOAT4 AS stars_mania 
FROM 
  scores 
  LEFT JOIN osu_scores_performance AS pp ON scores.score_id = pp.score_id 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty
  ) AS stars_osu ON scores.map_id = stars_osu.map_id 
  AND scores.mods = stars_osu.mods 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty_taiko
  ) AS stars_taiko ON scores.map_id = stars_taiko.map_id 
  AND scores.mods = stars_taiko.mods 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty_catch
  ) AS stars_catch ON scores.map_id = stars_catch.map_id 
  AND scores.mods = stars_catch.mods 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      osu_map_difficulty_mania
  ) AS stars_mania ON scores.map_id = stars_mania.map_id 
  AND scores.mods = stars_mania.mods 
ORDER BY 
  user_id, 
  scores.map_id, 
  gamemode, 
  scores.mods, 
  ended_at DESC"#,
            user_ids,
            country_code,
            mods_include,
            mods_exclude,
            mods_exact,
            map_id,
        );

        let mut scores = Vec::new();
        let mut rows = query.fetch(conn);

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("Failed to fetch next score")?;
            scores.push(DbScore::from(row));
        }

        Ok(scores)
    }

    // TODO: use builder pattern type
    #[allow(clippy::too_many_arguments)]
    pub async fn select_scores_by_discord_id<S>(
        &self,
        discord_users: &[i64],
        mode: Option<GameMode>,
        country_code: Option<&str>,
        map_id: Option<i32>,
        mods_include: Option<i32>,
        mods_exclude: Option<i32>,
        mods_exact: Option<i32>,
    ) -> Result<DbScores<S>>
    where
        S: BuildHasher + Default,
    {
        let mut conn = self
            .acquire()
            .await
            .wrap_err("Failed to acquire connection")?;

        let user_ids_query = sqlx::query!(
            r#"
SELECT 
osu_id 
FROM 
user_configs 
WHERE 
discord_id = ANY($1) 
AND osu_id IS NOT NULL"#,
            discord_users,
        );

        let mut user_ids = Vec::with_capacity(discord_users.len());

        {
            let mut rows = user_ids_query.fetch(&mut *conn);

            while let Some(row_res) = rows.next().await {
                let row = row_res.wrap_err("Failed to fetch next user id")?;
                user_ids.push(row.osu_id.expect("query ensures osu_id is not null"));
            }
        }

        Self::select_scores_by_osu_id_(
            &mut conn,
            &user_ids,
            mode,
            country_code,
            map_id,
            mods_include,
            mods_exclude,
            mods_exact,
        )
        .await
    }

    // TODO: use builder pattern type
    #[allow(clippy::too_many_arguments)]
    pub async fn select_scores_by_osu_id<S>(
        &self,
        user_ids: &[i32],
        mode: Option<GameMode>,
        country_code: Option<&str>,
        map_id: Option<i32>,
        mods_include: Option<i32>,
        mods_exclude: Option<i32>,
        mods_exact: Option<i32>,
    ) -> Result<DbScores<S>>
    where
        S: BuildHasher + Default,
    {
        let mut conn = self
            .acquire()
            .await
            .wrap_err("Failed to acquire connection")?;

        Self::select_scores_by_osu_id_(
            &mut conn,
            user_ids,
            mode,
            country_code,
            map_id,
            mods_include,
            mods_exclude,
            mods_exact,
        )
        .await
    }

    // TODO: use builder pattern type
    #[allow(clippy::too_many_arguments)]
    async fn select_scores_by_osu_id_<S>(
        conn: &mut PoolConnection<Postgres>,
        user_ids: &[i32],
        mode: Option<GameMode>,
        country_code: Option<&str>,
        map_id: Option<i32>,
        mods_include: Option<i32>,
        mods_exclude: Option<i32>,
        mods_exact: Option<i32>,
    ) -> Result<DbScores<S>>
    where
        S: BuildHasher + Default,
    {
        let scores = match mode {
            None => {
                Self::select_any_scores(
                    conn,
                    user_ids,
                    country_code,
                    map_id,
                    mods_include,
                    mods_exclude,
                    mods_exact,
                )
                .await?
            }
            Some(GameMode::Osu) => {
                Self::select_osu_scores(
                    conn,
                    user_ids,
                    country_code,
                    map_id,
                    mods_include,
                    mods_exclude,
                    mods_exact,
                )
                .await?
            }
            Some(GameMode::Taiko) => {
                Self::select_taiko_scores(
                    conn,
                    user_ids,
                    country_code,
                    map_id,
                    mods_include,
                    mods_exclude,
                    mods_exact,
                )
                .await?
            }
            Some(GameMode::Catch) => {
                Self::select_catch_scores(
                    conn,
                    user_ids,
                    country_code,
                    map_id,
                    mods_include,
                    mods_exclude,
                    mods_exact,
                )
                .await?
            }
            Some(GameMode::Mania) => {
                Self::select_mania_scores(
                    conn,
                    user_ids,
                    country_code,
                    map_id,
                    mods_include,
                    mods_exclude,
                    mods_exact,
                )
                .await?
            }
        };

        let map_ids: Vec<_> = scores.iter().map(|score| score.map_id as i32).collect();

        let map_query = sqlx::query_as!(
            DbScoreBeatmapRaw,
            r#"
SELECT 
  map_id, 
  mapset_id, 
  user_id, 
  map_version, 
  seconds_drain,  
  hp, 
  cs, 
  od, 
  ar, 
  bpm
FROM 
  osu_maps 
WHERE 
  map_id = ANY($1)"#,
            map_ids.as_slice()
        );

        let mut maps = HashMap::with_capacity_and_hasher(map_ids.len(), S::default());

        {
            let mut map_rows = map_query.fetch(&mut *conn);

            while let Some(row_res) = map_rows.next().await {
                let row = row_res.wrap_err("Failed to fetch next map")?;
                maps.insert(row.map_id as u32, row.into());
            }
        }

        let mapset_query = sqlx::query_as!(
            DbScoreBeatmapsetRaw,
            r#"
SELECT 
  mapsets.mapset_id, 
  artist, 
  title, 
  rank_status, 
  ranked_date 
FROM 
  (
    SELECT 
      mapset_id 
    FROM 
      osu_maps 
    WHERE 
      map_id = ANY($1)
  ) AS maps 
  JOIN (
    SELECT 
      * 
    FROM 
      osu_mapsets
  ) AS mapsets ON maps.mapset_id = mapsets.mapset_id"#,
            map_ids.as_slice()
        );

        let mut mapsets = HashMap::with_hasher(S::default());

        {
            let mut mapset_rows = mapset_query.fetch(&mut *conn);

            while let Some(row_res) = mapset_rows.next().await {
                let row = row_res.wrap_err("Failed to fetch next mapset")?;
                mapsets.insert(row.mapset_id as u32, row.into());
            }
        }

        let user_query = sqlx::query_as!(
            DbScoreUserRaw,
            r#"
SELECT 
  user_id, 
  username 
FROM 
  osu_user_names 
WHERE 
  user_id = ANY($1)"#,
            user_ids
        );

        let mut users = HashMap::with_hasher(S::default());

        {
            let mut user_rows = user_query.fetch(conn);

            while let Some(row_res) = user_rows.next().await {
                let row = row_res.wrap_err("Failed to fetch next user")?;
                users.insert(row.user_id as u32, row.into());
            }
        }

        Ok(DbScores {
            scores,
            maps,
            mapsets,
            users,
        })
    }

    pub async fn insert_scores(&self, scores: &[Score]) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("failed to begin transaction")?;

        for score in scores {
            let Score {
                accuracy: _,
                ended_at,
                grade,
                map_id,
                max_combo,
                map: _, // updated through checksum-missmatch
                mapset,
                mode,
                mods,
                passed: _,
                perfect,
                pp,
                rank_country: _,
                rank_global: _,
                replay: _,
                score,
                score_id,
                statistics:
                    ScoreStatistics {
                        count_geki,
                        count_300,
                        count_katu,
                        count_100,
                        count_50,
                        count_miss,
                    },
                user: _,
                user_id,
                weight: _,
            } = score;

            let Some(score_id) = score_id else { continue };

            let query = sqlx::query!(
                r#"
INSERT INTO osu_scores (
  score_id, user_id, map_id, gamemode, 
  mods, score, maxcombo, grade, count50, 
  count100, count300, countmiss, countgeki, 
  countkatu, perfect, ended_at
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16
  ) ON CONFLICT (score_id) DO NOTHING"#,
                *score_id as i64,
                *user_id as i32,
                *map_id as i32,
                *mode as i16,
                mods.bits() as i32,
                *score as i64,
                *max_combo as i32,
                *grade as i16,
                *count_50 as i32,
                *count_100 as i32,
                *count_300 as i32,
                *count_miss as i32,
                *count_geki as i32,
                *count_katu as i32,
                perfect,
                ended_at,
            );

            query
                .execute(&mut tx)
                .await
                .wrap_err("failed to execute score query")?;

            if let Some(pp) = pp {
                let query = sqlx::query!(
                    r#"
INSERT INTO osu_scores_performance (score_id, pp) 
VALUES 
  ($1, $2) ON CONFLICT (score_id) DO NOTHING"#,
                    *score_id as i64,
                    *pp as f64,
                );

                query
                    .execute(&mut tx)
                    .await
                    .wrap_err("failed to execute pp query")?;
            }

            if let Some(mapset) = mapset {
                Self::update_beatmapset_compact(&mut tx, mapset)
                    .await
                    .wrap_err("failed to update mapset")?;
            }
        }

        tx.commit().await.wrap_err("failed to commit transaction")?;

        Ok(())
    }

    pub async fn update_beatmapsets_compact(&self, scores: &[Score]) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("Failed to begin transaction")?;

        for score in scores {
            if let Some(ref mapset) = score.mapset {
                Self::update_beatmapset_compact(&mut tx, mapset).await?;
            }
        }

        tx.commit().await.wrap_err("Failed to commit transaction")?;

        Ok(())
    }

    pub async fn delete_scores_by_user_id<'c, E>(executor: E, user_id: u32) -> Result<()>
    where
        E: Executor<'c, Database = Postgres>,
    {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  osu_scores 
WHERE 
  user_id = $1"#,
            user_id as i32
        );

        query
            .execute(executor)
            .await
            .wrap_err("Failed to execute scores query")?;

        Ok(())
    }
}

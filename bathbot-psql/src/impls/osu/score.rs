use std::{cmp, collections::HashMap, hash::BuildHasher};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use rosu_v2::prelude::{GameMode, Grade, LegacyScoreStatistics, Score};
use sqlx::{pool::PoolConnection, Executor, Postgres};

use crate::{
    database::Database,
    model::osu::{
        DbScore, DbScoreAny, DbScoreBeatmapRaw, DbScoreBeatmapsetRaw, DbScoreCatch, DbScoreMania,
        DbScoreOsu, DbScoreTaiko, DbScoreUserRaw, DbScores, DbScoresBuilder, DbTopScore,
        DbTopScoreRaw, DbTopScores,
    },
};

macro_rules! select_scores {
    ( $fn:ident, $ty:ident, $mode:ident: $query:literal ) => {
        async fn $fn(
            conn: &mut PoolConnection<Postgres>,
            user_ids: &[i32],
            data: &DbScoresBuilder<'_>,
        ) -> Result<Vec<DbScore>> {
            let query = sqlx::query_as!(
                $ty,
                $query,
                user_ids,
                data.country_code,
                data.mods_include,
                data.mods_exclude,
                data.mods_exact,
                data.map_id,
                data.grade.map_or_else(Vec::new, |grade| match grade {
                    Grade::S => vec![Grade::S as i16, Grade::SH as i16],
                    Grade::X => vec![Grade::X as i16, Grade::XH as i16],
                    other => vec![other as i16],
                }) as _,
            );

            let mut scores = Vec::new();
            let mut rows = query.fetch(&mut **conn);

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
    AND (
      -- grade
      CARDINALITY($7 :: INT2[]) = 0 
      OR grade = ANY($7)
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  scores.score_id, 
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
    AND (
      -- grade
      CARDINALITY($7 :: INT2[]) = 0 
      OR grade = ANY($7)
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  scores.score_id, 
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
    AND (
      -- grade
      CARDINALITY($7 :: INT2[]) = 0 
      OR grade = ANY($7)
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  scores.score_id, 
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
    AND (
      -- grade
      CARDINALITY($7 :: INT2[]) = 0 
      OR grade = ANY($7)
    )
) 
SELECT 
  DISTINCT ON (
    user_id, scores.map_id, scores.mods
  ) user_id, 
  scores.map_id, 
  scores.mods, 
  score, 
  scores.score_id, 
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
        data: &DbScoresBuilder<'_>,
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
    AND (
      -- grade
      CARDINALITY($7 :: INT2[]) = 0 
      OR grade = ANY($7)
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
  scores.score_id, 
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
            data.country_code,
            data.mods_include,
            data.mods_exclude,
            data.mods_exact,
            data.map_id,
            data.grade.map_or_else(Vec::new, |grade| match grade {
                Grade::S => vec![Grade::S as i16, Grade::SH as i16],
                Grade::X => vec![Grade::X as i16, Grade::XH as i16],
                other => vec![other as i16],
            }) as _,
        );

        let mut scores = Vec::new();
        let mut rows = query.fetch(&mut **conn);

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("Failed to fetch next score")?;
            scores.push(DbScore::from(row));
        }

        Ok(scores)
    }

    pub(crate) async fn select_scores_by_discord_id<S>(
        &self,
        users: &[i64],
        data: &DbScoresBuilder<'_>,
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
            users,
        );

        let mut user_ids = Vec::with_capacity(users.len());

        {
            let mut rows = user_ids_query.fetch(&mut *conn);

            while let Some(row_res) = rows.next().await {
                let row = row_res.wrap_err("Failed to fetch next user id")?;
                user_ids.push(row.osu_id.expect("query ensures osu_id is not null"));
            }
        }

        Self::select_scores_by_osu_id_(&mut conn, &user_ids, data).await
    }

    pub(crate) async fn select_scores_by_osu_id<S>(
        &self,
        user_ids: &[i32],
        data: &DbScoresBuilder<'_>,
    ) -> Result<DbScores<S>>
    where
        S: BuildHasher + Default,
    {
        let mut conn = self
            .acquire()
            .await
            .wrap_err("Failed to acquire connection")?;

        Self::select_scores_by_osu_id_(&mut conn, user_ids, data).await
    }

    async fn select_scores_by_osu_id_<S>(
        conn: &mut PoolConnection<Postgres>,
        user_ids: &[i32],
        data: &DbScoresBuilder<'_>,
    ) -> Result<DbScores<S>>
    where
        S: BuildHasher + Default,
    {
        let scores = match data.mode {
            None => Self::select_any_scores(conn, user_ids, data).await?,
            Some(GameMode::Osu) => Self::select_osu_scores(conn, user_ids, data).await?,
            Some(GameMode::Taiko) => Self::select_taiko_scores(conn, user_ids, data).await?,
            Some(GameMode::Catch) => Self::select_catch_scores(conn, user_ids, data).await?,
            Some(GameMode::Mania) => Self::select_mania_scores(conn, user_ids, data).await?,
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
            let mut map_rows = map_query.fetch(&mut **conn);

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
            let mut mapset_rows = mapset_query.fetch(&mut **conn);

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
            let mut user_rows = user_query.fetch(&mut **conn);

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

    pub async fn select_top100_scores<S>(
        &self,
        mode: GameMode,
        country_code: Option<&str>,
        user_ids: Option<&[i32]>,
    ) -> Result<DbTopScores<S>>
    where
        S: BuildHasher + Default,
    {
        let query = sqlx::query_as!(
            DbTopScoreRaw,
            r#"
WITH osu_stars AS (
  SELECT 
    map_id, 
    mods, 
    stars, 
    0 :: INT2 AS gamemode 
  FROM 
    osu_map_difficulty
), 
taiko_stars AS (
  SELECT 
    map_id, 
    mods, 
    stars, 
    1 :: INT2 AS gamemode 
  FROM 
    osu_map_difficulty_taiko
), 
catch_stars AS (
  SELECT 
    map_id, 
    mods, 
    stars, 
    2 :: INT2 AS gamemode 
  FROM 
    osu_map_difficulty_catch
), 
mania_stars AS (
  SELECT 
    map_id, 
    mods, 
    stars, 
    3 :: INT2 AS gamemode 
  FROM 
    osu_map_difficulty_mania
) 
SELECT 
  username, 
  user_id AS "user_id!: _", 
  map_id AS "map_id!: _", 
  mods AS "mods!: _", 
  score AS "score!: _", 
  score_id AS "score_id!: _", 
  maxcombo AS "maxcombo!: _", 
  grade AS "grade!: _", 
  count50 AS "count50!: _", 
  count100 AS "count100!: _", 
  count300 AS "count300!: _", 
  countgeki AS "countgeki!: _", 
  countkatu AS "countkatu!: _", 
  countmiss AS "countmiss!: _", 
  ended_at AS "ended_at!: _", 
  pp :: FLOAT4 AS "pp!: _", 
  stars :: FLOAT4 
FROM 
  (
    SELECT 
      DISTINCT ON (user_id, map_id) limited_user_scores.*, 
      osu_user_names.username 
    FROM 
      (
        SELECT 
          * 
        FROM 
          user_scores 
        WHERE 
          gamemode = $1 
          AND (
            $2 :: INT4[] IS NULL 
            OR user_id = ANY($2)
          ) 
          AND (
            $3 :: VARCHAR(2) IS NULL 
            OR country_code = $3
          ) 
        ORDER BY 
          pp DESC 
        LIMIT 
          1000
      ) as limited_user_scores 
      JOIN osu_user_names USING (user_id) 
    ORDER BY 
      user_id, 
      map_id, 
      pp DESC
  ) AS scores 
  LEFT JOIN (
    SELECT 
      map_id, 
      mods, 
      stars 
    FROM 
      (
        SELECT 
          * 
        FROM 
          osu_stars 
        UNION ALL 
        SELECT 
          * 
        FROM 
          taiko_stars 
        UNION ALL 
        SELECT 
          * 
        FROM 
          catch_stars 
        UNION ALL 
        SELECT 
          * 
        FROM 
          mania_stars
      ) AS stars_union 
    WHERE 
      gamemode = $1
  ) AS stars USING (map_id, mods) 
ORDER BY 
  pp DESC 
LIMIT 
  100"#,
            mode as i16,
            user_ids,
            country_code,
        );

        let mut conn = self
            .acquire()
            .await
            .wrap_err("Failed to acquire connection")?;

        let mut scores = Vec::new();

        {
            let mut score_rows = query.fetch(&mut *conn);
            let mut pos = 1;

            while let Some(row_res) = score_rows.next().await {
                let row = row_res.wrap_err("Failed to fetch next score")?;
                scores.push(DbTopScore::new(row, pos, mode));
                pos += 1;
            }
        }

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

        Ok(DbTopScores {
            scores,
            maps,
            mapsets,
        })
    }

    pub async fn insert_scores(&self, scores: &[Score]) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("failed to begin transaction")?;

        let cap = cmp::min(scores.len(), 100);

        let mut vec_score_id = Vec::with_capacity(cap);
        let mut vec_user_id = Vec::with_capacity(cap);
        let mut vec_map_id = Vec::with_capacity(cap);
        let mut vec_gamemode = Vec::with_capacity(cap);
        let mut vec_mods = Vec::with_capacity(cap);
        let mut vec_score = Vec::with_capacity(cap);
        let mut vec_maxcombo = Vec::with_capacity(cap);
        let mut vec_grade = Vec::with_capacity(cap);
        let mut vec_count50 = Vec::with_capacity(cap);
        let mut vec_count100 = Vec::with_capacity(cap);
        let mut vec_count300 = Vec::with_capacity(cap);
        let mut vec_countmiss = Vec::with_capacity(cap);
        let mut vec_countgeki = Vec::with_capacity(cap);
        let mut vec_countkatu = Vec::with_capacity(cap);
        let mut vec_perfect = Vec::with_capacity(cap);
        let mut vec_ended_at = Vec::with_capacity(cap);
        let mut vec_pp_score_id = Vec::with_capacity(cap);
        let mut vec_pp = Vec::with_capacity(cap);

        for chunk in scores.chunks(100) {
            vec_score_id.clear();
            vec_user_id.clear();
            vec_map_id.clear();
            vec_gamemode.clear();
            vec_mods.clear();
            vec_score.clear();
            vec_maxcombo.clear();
            vec_grade.clear();
            vec_count50.clear();
            vec_count100.clear();
            vec_count300.clear();
            vec_countmiss.clear();
            vec_countgeki.clear();
            vec_countkatu.clear();
            vec_perfect.clear();
            vec_ended_at.clear();
            vec_pp_score_id.clear();
            vec_pp.clear();

            for score in chunk {
                let Score {
                    set_on_lazer: _, // TODO: probably want to use this
                    classic_score: _,
                    ranked: _,
                    preserve: _,
                    processed: _,
                    maximum_statistics: _,
                    mods,
                    statistics,
                    map_id,
                    best_id: _,
                    id: score_id,
                    grade,
                    kind: _,
                    user_id,
                    accuracy: _,
                    build_id: _,
                    ended_at,
                    has_replay: _,
                    is_perfect_combo,
                    legacy_perfect,
                    legacy_score_id,
                    legacy_score: _,
                    max_combo,
                    passed,
                    pp,
                    mode,
                    started_at: _,
                    score,
                    replay: _,
                    current_user_attributes: _,
                    total_score_without_mods: _,
                    map: _,    // updated through checksum-missmatch
                    mapset: _, // handled in later iteration further down
                    rank_global: _,
                    user: _,
                    weight: _,
                } = score;

                // TODO: remove from database?
                let perfect = legacy_perfect.unwrap_or(*is_perfect_combo);

                let grade = if *passed { *grade } else { Grade::F };
                let score_id = legacy_score_id.unwrap_or(*score_id);

                let LegacyScoreStatistics {
                    count_geki,
                    count_katu,
                    count_300,
                    count_100,
                    count_50,
                    count_miss,
                } = statistics.as_legacy(*mode);

                vec_score_id.push(score_id as i64);
                vec_user_id.push(*user_id as i32);
                vec_map_id.push(*map_id as i32);
                vec_gamemode.push(*mode as i16);
                vec_mods.push(mods.bits() as i32);
                vec_score.push(*score as i32);
                vec_maxcombo.push(*max_combo as i32);
                vec_grade.push(grade as i16);
                vec_count50.push(count_50 as i32);
                vec_count100.push(count_100 as i32);
                vec_count300.push(count_300 as i32);
                vec_countmiss.push(count_miss as i32);
                vec_countgeki.push(count_geki as i32);
                vec_countkatu.push(count_katu as i32);
                vec_perfect.push(perfect);
                vec_ended_at.push(*ended_at);

                if let Some(pp) = pp {
                    vec_pp_score_id.push(score_id as i64);
                    vec_pp.push(*pp as f64);
                }
            }

            let query = sqlx::query!(
                r#"
INSERT INTO osu_scores (
score_id, user_id, map_id, gamemode, 
mods, score, maxcombo, grade, count50, 
count100, count300, countmiss, countgeki, 
countkatu, perfect, ended_at
) 
SELECT
  *
FROM
  UNNEST(
  $1::INT8[], $2::INT4[], $3::INT4[], $4::INT2[], 
  $5::INT4[], $6::INT4[], $7::INT4[], $8::INT2[], 
  $9::INT4[], $10::INT4[], $11::INT4[], $12::INT4[], 
  $13::INT4[], $14::INT4[], $15::BOOL[], $16::TIMESTAMPTZ[]
) ON CONFLICT (score_id) DO NOTHING"#,
                &vec_score_id,
                &vec_user_id,
                &vec_map_id,
                &vec_gamemode,
                &vec_mods,
                &vec_score,
                &vec_maxcombo,
                &vec_grade,
                &vec_count50,
                &vec_count100,
                &vec_count300,
                &vec_countmiss,
                &vec_countgeki,
                &vec_countkatu,
                &vec_perfect,
                &vec_ended_at,
            );

            query
                .execute(&mut *tx)
                .await
                .wrap_err("failed to execute score query")?;

            if !vec_pp.is_empty() {
                let query = sqlx::query!(
                    r#"
INSERT INTO osu_scores_performance (score_id, pp) 
SELECT
  *
FROM
  UNNEST($1::INT8[], $2::FLOAT8[]) ON CONFLICT (score_id) DO NOTHING"#,
                    &vec_pp_score_id,
                    &vec_pp,
                );

                query
                    .execute(&mut *tx)
                    .await
                    .wrap_err("failed to execute pp query")?;
            }

            let mapset_iter = chunk.iter().filter_map(|score| score.mapset.as_deref());

            Self::update_beatmapsets(&mut *tx, mapset_iter, chunk.len())
                .await
                .wrap_err("failed to update mapset")?
        }

        tx.commit().await.wrap_err("failed to commit transaction")?;

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

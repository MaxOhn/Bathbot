use std::{cmp::Ordering, collections::HashMap, hash::BuildHasher, mem};

use bathbot_model::{UserModeStatsColumn, UserStatsColumn, UserStatsEntries, UserStatsEntry};
use eyre::{Result, WrapErr};
use futures::StreamExt;
use rosu_v2::prelude::{GameMode, UserExtended, Username};
use time::OffsetDateTime;

use crate::{
    model::osu::{DbUserStatsEntry, OsuUserStatsColumnName},
    Database,
};

fn convert_entries<V>(entries: Vec<DbUserStatsEntry<V>>) -> Vec<UserStatsEntry<V>> {
    // SAFETY: the two types have the exact same structure
    unsafe { mem::transmute(entries) }
}

impl Database {
    pub async fn select_osu_user_stats(
        &self,
        discord_ids: &[i64],
        column: UserStatsColumn,
        country_code: Option<&str>,
    ) -> Result<UserStatsEntries> {
        let query = format!(
            r#"
SELECT 
  username, 
  country_code, 
  {column} AS value 
FROM 
  (
    SELECT 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      discord_id = ANY($1) 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN osu_user_names AS names ON configs.osu_id = names.user_id 
  JOIN (
    SELECT 
      user_id, 
      country_code, 
      {column} 
    FROM 
      osu_user_stats
    WHERE 
      $2 :: VARCHAR(2) is NULL 
      OR country_code = $2
  ) AS stats ON names.user_id = stats.user_id"#,
            column = column.column(),
        );

        match column {
            UserStatsColumn::Badges
            | UserStatsColumn::Comments
            | UserStatsColumn::Followers
            | UserStatsColumn::ForumPosts
            | UserStatsColumn::GraveyardMapsets
            | UserStatsColumn::KudosuAvailable
            | UserStatsColumn::KudosuTotal
            | UserStatsColumn::LovedMapsets
            | UserStatsColumn::Subscribers
            | UserStatsColumn::Medals
            | UserStatsColumn::PlayedMaps
            | UserStatsColumn::RankedMapsets
            | UserStatsColumn::Namechanges => {
                let mut entries: Vec<DbUserStatsEntry<i32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as u64,
                    })
                    .collect();

                Ok(UserStatsEntries::Amount(entries))
            }
            UserStatsColumn::JoinDate => {
                let mut entries: Vec<DbUserStatsEntry<OffsetDateTime>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    a.value.cmp(&b.value).then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                Ok(UserStatsEntries::Date(convert_entries(entries)))
            }
        }
    }

    pub async fn select_osu_user_mode_stats(
        &self,
        discord_ids: &[i64],
        mode: GameMode,
        column: UserModeStatsColumn,
        country_code: Option<&str>,
    ) -> Result<UserStatsEntries> {
        fn default_query(column: &str) -> String {
            format!(
                r#"
SELECT 
  username, 
  country_code, 
  value 
FROM 
  (
    SELECT 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      discord_id = ANY($1) 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN osu_user_names AS names ON configs.osu_id = names.user_id 
  JOIN (
    SELECT 
      user_id, 
      {column} AS value 
    FROM 
      osu_user_mode_stats 
    WHERE 
      gamemode = $2
  ) AS stats ON names.user_id = stats.user_id 
  JOIN (
    SELECT 
      user_id, 
      country_code 
    FROM 
      osu_user_stats
    WHERE 
      $3 :: VARCHAR(2) is NULL 
      OR country_code = $3
  ) AS country ON names.user_id = country.user_id"#
            )
        }

        match column {
            UserModeStatsColumn::Accuracy => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<f32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value
                        .partial_cmp(&a.value)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                Ok(UserStatsEntries::Accuracy(convert_entries(entries)))
            }
            UserModeStatsColumn::AverageHits => {
                let query = r#"
SELECT 
  username, 
  country_code, 
  GREATEST(total_hits::FLOAT4 / NULLIF(playcount::FLOAT4, 0), 0) AS value 
FROM 
  (
    SELECT 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      discord_id = ANY($1) 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN osu_user_names AS names ON configs.osu_id = names.user_id 
  JOIN (
    SELECT 
      user_id, 
      total_hits, 
      playcount 
    FROM 
      osu_user_mode_stats 
    WHERE 
      gamemode = $2
  ) AS stats ON names.user_id = stats.user_id 
  JOIN (
    SELECT 
      user_id, 
      country_code 
    FROM 
      osu_user_stats
    WHERE 
      $3 :: VARCHAR(2) is NULL 
      OR country_code = $3
  ) AS country ON names.user_id = country.user_id"#;

                let mut entries: Vec<DbUserStatsEntry<f32>> = sqlx::query_as(query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value
                        .partial_cmp(&a.value)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                Ok(UserStatsEntries::Float(convert_entries(entries)))
            }
            UserModeStatsColumn::CountSsh
            | UserModeStatsColumn::CountSs
            | UserModeStatsColumn::CountSh
            | UserModeStatsColumn::CountS
            | UserModeStatsColumn::CountA => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<i32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as i64,
                    })
                    .collect();

                Ok(UserStatsEntries::AmountWithNegative(entries))
            }
            UserModeStatsColumn::TotalSs => {
                let query = r#"
SELECT 
  username, 
  country_code, 
  COALESCE(count_ssh, 0) + COALESCE(count_ss, 0) AS value 
FROM 
  (
    SELECT 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      discord_id = ANY($1) 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN osu_user_names AS names ON configs.osu_id = names.user_id 
  JOIN (
    SELECT 
      user_id, 
      count_ssh, 
      count_ss 
    FROM 
      osu_user_mode_stats 
    WHERE 
      gamemode = $2
  ) AS stats ON names.user_id = stats.user_id 
  JOIN (
    SELECT 
      user_id, 
      country_code 
    FROM 
      osu_user_stats
    WHERE 
      $3 :: VARCHAR(2) is NULL 
      OR country_code = $3
  ) AS country ON names.user_id = country.user_id"#;

                let mut entries: Vec<DbUserStatsEntry<i32>> = sqlx::query_as(query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value
                        .partial_cmp(&a.value)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as i64,
                    })
                    .collect();

                Ok(UserStatsEntries::AmountWithNegative(entries))
            }
            UserModeStatsColumn::TotalS => {
                let query = r#"
SELECT 
  username, 
  country_code, 
  COALESCE(count_sh, 0) + COALESCE(count_s, 0) AS value 
FROM 
  (
    SELECT 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      discord_id = ANY($1) 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN osu_user_names AS names ON configs.osu_id = names.user_id 
  JOIN (
    SELECT 
      user_id, 
      count_sh, 
      count_s 
    FROM 
      osu_user_mode_stats 
    WHERE 
      gamemode = $2
  ) AS stats ON names.user_id = stats.user_id 
  JOIN (
    SELECT 
      user_id, 
      country_code 
    FROM 
      osu_user_stats
    WHERE 
      $3 :: VARCHAR(2) is NULL 
      OR country_code = $3
  ) AS country ON names.user_id = country.user_id"#;

                let mut entries: Vec<DbUserStatsEntry<i32>> = sqlx::query_as(query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value
                        .partial_cmp(&a.value)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as i64,
                    })
                    .collect();

                Ok(UserStatsEntries::AmountWithNegative(entries))
            }
            UserModeStatsColumn::Level => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<f32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value
                        .partial_cmp(&a.value)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                Ok(UserStatsEntries::Float(convert_entries(entries)))
            }
            UserModeStatsColumn::Playtime => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<i32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as u32,
                    })
                    .collect();

                Ok(UserStatsEntries::Playtime(entries))
            }
            UserModeStatsColumn::Pp => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<f32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("Failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value
                        .partial_cmp(&a.value)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                Ok(UserStatsEntries::PpF32(convert_entries(entries)))
            }
            UserModeStatsColumn::PpPerMonth => {
                let query = r#"
SELECT 
username, 
country_code, 
GREATEST((30.67 * pp / NULLIF(EXTRACT(DAYS FROM (NOW() - join_date)), 0))::FLOAT4, 0) AS value 
FROM 
(
  SELECT 
    osu_id 
  FROM 
    user_configs 
  WHERE 
    discord_id = ANY($1) 
    AND osu_id IS NOT NULL
) AS configs 
JOIN osu_user_names AS names ON configs.osu_id = names.user_id 
JOIN (
  SELECT 
    user_id, 
    pp 
  FROM 
    osu_user_mode_stats 
  WHERE 
    gamemode = $2
) AS stats ON names.user_id = stats.user_id 
JOIN (
  SELECT 
    user_id, 
    country_code, 
    join_date 
  FROM 
    osu_user_stats
  WHERE 
    $3 :: VARCHAR(2) is NULL 
    OR country_code = $3
) AS country ON names.user_id = country.user_id"#;

                let mut entries: Vec<DbUserStatsEntry<f32>> = sqlx::query_as(query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("Failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value
                        .partial_cmp(&a.value)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                Ok(UserStatsEntries::PpF32(convert_entries(entries)))
            }
            UserModeStatsColumn::RankCountry | UserModeStatsColumn::RankGlobal => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<i32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                // Could be handled in the query already
                // * Filter out inactive players
                entries.retain(|entry| entry.value != 0);

                entries.sort_unstable_by(|a, b| {
                    a.value.cmp(&b.value).then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as u32,
                    })
                    .collect();

                Ok(UserStatsEntries::Rank(entries))
            }
            UserModeStatsColumn::MaxCombo
            | UserModeStatsColumn::Playcount
            | UserModeStatsColumn::ReplaysWatched
            | UserModeStatsColumn::ScoresFirst => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<i32>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as u64,
                    })
                    .collect();

                Ok(UserStatsEntries::Amount(entries))
            }
            UserModeStatsColumn::ScoreRanked
            | UserModeStatsColumn::ScoreTotal
            | UserModeStatsColumn::TotalHits => {
                let query = default_query(column.column().unwrap());

                let mut entries: Vec<DbUserStatsEntry<i64>> = sqlx::query_as(&query)
                    .bind(discord_ids)
                    .bind(mode as i16)
                    .bind(country_code)
                    .fetch_all(self)
                    .await
                    .wrap_err("failed to fetch all")?;

                entries.sort_unstable_by(|a, b| {
                    b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name))
                });

                entries.dedup_by(|a, b| a.name == b.name);

                let entries = entries
                    .into_iter()
                    .map(|entry| UserStatsEntry {
                        country: entry.country,
                        name: entry.name,
                        value: entry.value as u64,
                    })
                    .collect();

                Ok(UserStatsEntries::Amount(entries))
            }
        }
    }

    /// Be sure wildcards (_, %) are escaped as required!
    pub async fn select_osu_user_ids(&self, names: &[String]) -> Result<HashMap<Username, u32>> {
        let query = sqlx::query!(
            r#"
SELECT 
user_id, 
username 
from 
osu_user_names 
WHERE 
username ILIKE ANY($1)"#,
            names
        );

        let mut rows = query.fetch(self);
        let mut ids = HashMap::with_capacity(names.len());

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("failed to fetch next")?;
            ids.insert(row.username.into(), row.user_id as u32);
        }

        Ok(ids)
    }

    pub async fn select_osu_usernames<S>(
        &self,
        user_ids: &[i32],
    ) -> Result<HashMap<u32, Username, S>>
    where
        S: Default + BuildHasher,
    {
        let query = sqlx::query!(
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

        let mut names = HashMap::with_capacity_and_hasher(user_ids.len(), S::default());
        let mut rows = query.fetch(self);

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("failed to fetch next")?;
            let user_id = row.user_id as u32;
            let username = row.username.into();
            names.insert(user_id, username);
        }

        Ok(names)
    }

    pub async fn upsert_osu_user(&self, user: &UserExtended, mode: GameMode) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("failed to begin transaction")?;

        let query = sqlx::query!(
            r#"
INSERT INTO osu_user_names (user_id, username) 
VALUES 
  ($1, $2) ON CONFLICT (user_id) DO 
UPDATE 
SET 
  username = $2"#,
            user.user_id as i32,
            user.username.as_str()
        );

        query
            .execute(&mut *tx)
            .await
            .wrap_err("failed to execute osu_user_names query")?;

        let query = sqlx::query!(
            r#"
INSERT INTO osu_user_stats (
  user_id, country_code, join_date, 
  comment_count, kudosu_total, kudosu_available, 
  forum_post_count, badges, played_maps, 
  followers, graveyard_mapset_count, 
  loved_mapset_count, mapping_followers, 
  previous_usernames_count, ranked_mapset_count, 
  medals
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16
  ) ON CONFLICT (user_id) DO 
UPDATE 
SET 
  country_code = $2, 
  comment_count = $4, 
  kudosu_total = $5, 
  kudosu_available = $6, 
  forum_post_count = $7, 
  badges = $8, 
  played_maps = $9, 
  followers = $10, 
  graveyard_mapset_count = $11, 
  loved_mapset_count = $12, 
  mapping_followers = $13, 
  previous_usernames_count = $14, 
  ranked_mapset_count = $15, 
  medals = $16,
  last_update = NOW()"#,
            user.user_id as i32,
            user.country_code.as_str(),
            user.join_date,
            user.comments_count as i32,
            user.kudosu.total,
            user.kudosu.available,
            user.forum_post_count as i32,
            user.badges.as_ref().map_or(0, Vec::len) as i32,
            user.beatmap_playcounts_count.unwrap_or(0) as i32,
            user.follower_count.unwrap_or(0) as i32,
            user.graveyard_mapset_count.unwrap_or(0) as i32,
            user.loved_mapset_count.unwrap_or(0) as i32,
            user.mapping_follower_count.unwrap_or(0) as i32,
            user.previous_usernames.as_ref().map_or(0, Vec::len) as i32,
            user.ranked_mapset_count.unwrap_or(0) as i32,
            user.medals.as_ref().map_or(0, Vec::len) as i32,
        );

        query
            .execute(&mut *tx)
            .await
            .wrap_err("failed to execute osu_user_stats query")?;

        if let Some(ref stats) = user.statistics {
            let query = sqlx::query!(
                r#"
INSERT INTO osu_user_mode_stats (
  user_id, gamemode, accuracy, pp, country_rank, 
  global_rank, count_ss, count_ssh, 
  count_s, count_sh, count_a, user_level, 
  max_combo, playcount, playtime, ranked_score, 
  replays_watched, total_hits, total_score, 
  scores_first
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16, $17, $18, 
    $19, $20
  ) ON CONFLICT (user_id, gamemode) DO 
UPDATE 
SET 
  accuracy = $3, 
  pp = $4, 
  country_rank = $5, 
  global_rank = $6, 
  count_ss = $7, 
  count_ssh = $8, 
  count_s = $9, 
  count_sh = $10, 
  count_a = $11, 
  user_level = $12, 
  max_combo = $13, 
  playcount = $14, 
  playtime = $15, 
  ranked_score = $16, 
  replays_watched = $17, 
  total_hits = $18, 
  total_score = $19, 
  scores_first = $20,
  last_update = NOW()"#,
                user.user_id as i32,
                mode as i16,
                stats.accuracy,
                stats.pp,
                stats.country_rank.unwrap_or(0) as i32,
                stats.global_rank.unwrap_or(0) as i32,
                stats.grade_counts.ss,
                stats.grade_counts.ssh,
                stats.grade_counts.s,
                stats.grade_counts.sh,
                stats.grade_counts.a,
                stats.level.current as f32 + stats.level.progress as f32 / 100.0,
                stats.max_combo as i32,
                stats.playcount as i32,
                stats.playtime as i32,
                stats.ranked_score as i64,
                stats.replays_watched as i32,
                stats.total_hits as i64,
                stats.total_score as i64,
                user.scores_first_count.unwrap_or(0) as i32,
            );

            query
                .execute(&mut *tx)
                .await
                .wrap_err("failed to execute osu_user_mode_stats query")?;
        }

        tx.commit().await.wrap_err("failed to commit transaction")?;

        Ok(())
    }

    pub async fn delete_osu_user_stats_and_scores(&self, user_id: u32) -> Result<()> {
        let mut conn = self
            .acquire()
            .await
            .wrap_err("Failed to acquire connection")?;

        let query = sqlx::query!(
            r#"
DELETE FROM 
  osu_user_stats 
WHERE 
  user_id = $1"#,
            user_id as i32
        );

        query
            .execute(&mut *conn)
            .await
            .wrap_err("Failed to execute osu_user_stats query")?;

        let query = sqlx::query!(
            r#"
DELETE FROM 
  osu_user_mode_stats 
WHERE 
  user_id = $1"#,
            user_id as i32
        );

        query
            .execute(&mut *conn)
            .await
            .wrap_err("Failed to execute osu_user_mode_stats query")?;

        Self::delete_osu_username(&mut *conn, user_id).await?;

        Ok(())
    }
}

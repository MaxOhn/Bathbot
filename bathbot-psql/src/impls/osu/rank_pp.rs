use eyre::{Report, Result, WrapErr};
use futures::StreamExt;
use rosu_v2::prelude::GameMode;

use crate::database::Database;

impl Database {
    pub async fn select_rank_approx_by_pp(&self, pp: f32, mode: GameMode) -> Result<u32> {
        let query = sqlx::query!(
            r#"
WITH stats AS (
  SELECT 
    global_rank, 
    pp, 
    last_update 
  FROM 
    osu_user_mode_stats 
  WHERE 
    gamemode = $1 
    AND NOW() - last_update < interval '2 days'
) 
SELECT 
  * 
FROM 
  (
    (
      SELECT 
        global_rank, 
        pp 
      FROM 
        (
          SELECT 
            * 
          FROM 
            stats 
          WHERE 
            pp >= $2 
          ORDER BY 
            pp ASC 
          LIMIT 
            2
        ) AS innerTable 
      ORDER BY 
        last_update DESC 
      LIMIT 
        1
    ) 
    UNION ALL 
      (
        SELECT 
          global_rank, 
          pp 
        FROM 
          (
            SELECT 
              * 
            FROM 
              stats 
            WHERE 
              pp <= $2 
            ORDER BY 
              pp DESC 
            LIMIT 
              2
          ) AS innerTable 
        ORDER BY 
          last_update DESC 
        LIMIT 
          1
      )
  ) AS neighbors"#,
            mode as i16,
            pp,
        );

        let mut rows = query.fetch(self);

        let (higher_rank, higher_pp) = match rows.next().await {
            Some(Ok(row)) => {
                let rank = row.global_rank.unwrap_or(0) as u32;
                let pp = row.pp.unwrap_or(0.0) as f32;

                (rank, pp)
            }
            Some(Err(err)) => return Err(Report::new(err).wrap_err("failed to get high pp")),
            None => return Ok(0),
        };

        let lower = rows
            .next()
            .await
            .transpose()
            .wrap_err("failed to get lower")?
            .map(|row| {
                let rank = row.global_rank.unwrap_or(0) as u32;
                let pp = row.pp.unwrap_or(0.0) as f32;

                (rank, pp)
            });

        trace!("PP={pp} => high: ({higher_rank}, {higher_pp}) | low: {lower:?}");

        if let Some((lower_rank, lower_pp)) = lower {
            ensure!(
                (lower_pp..=higher_pp).contains(&pp),
                "{pp}pp is not between {lower_pp} and {higher_pp}"
            );

            if (higher_pp - lower_pp).abs() <= f32::EPSILON {
                Ok(lower_rank.min(higher_rank).saturating_sub(1))
            } else {
                let percent = (higher_pp - pp) / (higher_pp - lower_pp);
                let rank = percent * lower_rank.saturating_sub(higher_rank) as f32;

                Ok(higher_rank + rank as u32)
            }
        } else if higher_pp < pp {
            Ok(higher_rank)
        } else if higher_pp > pp || higher_rank > 0 {
            Ok(higher_rank + 1)
        } else {
            Ok(0)
        }
    }

    pub async fn select_pp_approx_by_rank(&self, rank: u32, mode: GameMode) -> Result<f32> {
        let query = sqlx::query!(
            r#"
WITH stats AS (
  SELECT 
    global_rank, 
    pp, 
    last_update 
  FROM 
    osu_user_mode_stats 
  WHERE 
    gamemode = $1 
    AND NOW() - last_update < interval '2 days'
) 
SELECT 
  * 
FROM 
  (
    (
      SELECT 
        global_rank, 
        pp 
      FROM 
        (
          SELECT 
            * 
          FROM 
            stats 
          WHERE 
            global_rank > 0 
            AND global_rank <= $2 
          ORDER BY 
            pp ASC 
          LIMIT 
            2
        ) AS innerTable 
      ORDER BY 
        last_update DESC 
      LIMIT 
        1
    ) 
    UNION ALL 
      (
        SELECT 
          global_rank, 
          pp 
        FROM 
          (
            SELECT 
              * 
            FROM 
              stats 
            WHERE 
              global_rank >= $2 
            ORDER BY 
              pp DESC 
            LIMIT 
              2
          ) AS innerTable 
        ORDER BY 
          last_update DESC 
        LIMIT 
          1
      )
  ) AS neighbors"#,
            mode as i16,
            rank as i32,
        );

        let mut rows = query.fetch(self);

        let (higher_rank, higher_pp) = match rows.next().await {
            Some(Ok(row)) => {
                let rank = row.global_rank.unwrap_or(0) as u32;
                let pp = row.pp.unwrap_or(0.0) as f32;

                (rank, pp)
            }
            Some(Err(err)) => return Err(Report::new(err).wrap_err("failed to get higher")),
            None => return Ok(0.0),
        };

        let lower = rows
            .next()
            .await
            .transpose()
            .wrap_err("failed to get lower")?
            .map(|row| {
                let rank = row.global_rank.unwrap_or(0) as u32;
                let pp = row.pp.unwrap_or(0.0) as f32;

                (rank, pp)
            });

        trace!("Rank {rank} => high: ({higher_rank}, {higher_pp}) | low: {lower:?}");

        if let Some((lower_rank, lower_pp)) = lower {
            ensure!(
                (higher_rank..=lower_rank).contains(&rank),
                "rank {rank} is not between {higher_rank} and {lower_rank}"
            );

            if lower_rank == higher_rank {
                Ok(lower_pp.max(higher_pp) + 0.01)
            } else {
                let percent = (lower_rank - rank) as f32 / (lower_rank - higher_rank) as f32;
                let pp = percent * (higher_pp - lower_pp).max(0.0);

                Ok(lower_pp + pp)
            }
        } else if higher_rank > rank {
            Ok(higher_pp + 0.01)
        } else if higher_rank < rank || higher_pp > 0.0 {
            Ok(higher_pp - 0.01)
        } else {
            Ok(0.0)
        }
    }
}

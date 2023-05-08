use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;

use crate::database::Database;

impl Database {
    pub async fn select_rank_approx_by_pp(&self, pp: f32, mode: GameMode) -> Result<u32> {
        let query = sqlx::query_as!(
            DbEntry,
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
    AND global_rank > 0 
    AND NOW() - last_update < interval '2 days'
) 
SELECT 
  * 
FROM 
  (
    (
      SELECT 
        global_rank, 
        pp, 
        0::INT2 AS pos 
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
            5
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
          pp, 
          1::INT2 AS pos 
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
              5
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

        let entries = query
            .fetch_all(self)
            .await
            .map(Entries::from)
            .wrap_err("Failed to fetch all entries")?;

        if let (Some(higher_pp), Some(lower_rank)) = (entries.higher_pp(), entries.lower_rank()) {
            // found a DB entry above and below the given pp

            let higher_rank = entries.higher_rank();
            let lower_pp = entries.lower_pp();

            ensure!(
                (lower_pp..=higher_pp).contains(&pp),
                "{pp}pp is not between {lower_pp} and {higher_pp}"
            );

            if lower_rank < higher_rank {
                // "lower" DB entry was actually higher due to either entry being outdated

                Ok(lower_rank)
            } else if (higher_pp - lower_pp).abs() <= f32::EPSILON {
                // both entries match the given pp exactly

                Ok(higher_rank)
            } else {
                // lerp

                let percent = (pp - lower_pp) / (higher_pp - lower_pp);
                let rank = percent * (lower_rank - higher_rank) as f32;

                Ok(lower_rank - rank as u32)
            }
        } else if entries.higher_pp().is_some() {
            // only a higher entry was available
            // e.g. given pp is below any stored pp

            Ok(entries.higher_rank() + 1)
        } else if let Some(lower_rank) = entries.lower_rank() {
            // only a lower entry was available
            // e.g. given pp is above any stored pp

            Ok(lower_rank)
        } else {
            Ok(0)
        }
    }

    pub async fn select_pp_approx_by_rank(&self, rank: u32, mode: GameMode) -> Result<f32> {
        let query = sqlx::query_as!(
            DbEntry,
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
    AND global_rank > 0 
    AND NOW() - last_update < interval '2 days'
) 
SELECT 
  * 
FROM 
  (
    (
      SELECT 
        global_rank, 
        pp, 
        0::INT2 AS pos 
      FROM 
        (
          SELECT 
            * 
          FROM 
            stats 
          WHERE 
            global_rank <= $2 
          ORDER BY 
            pp ASC 
          LIMIT 
            5
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
          pp, 
          1::INT2 AS pos 
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
              5
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

        let entries = query
            .fetch_all(self)
            .await
            .map(Entries::from)
            .wrap_err("Failed to fetch all entries")?;

        if let (Some(higher_pp), Some(lower_rank)) = (entries.higher_pp(), entries.lower_rank()) {
            // found a DB entry above and below the given rank

            let higher_rank = entries.higher_rank();
            let lower_pp = entries.lower_pp();

            ensure!(
                (higher_rank..=lower_rank).contains(&rank),
                "rank {rank} is not between {higher_rank} and {lower_rank}"
            );

            if lower_pp > higher_pp {
                // "lower" DB entry was actually higher due to either entry being outdated

                Ok(lower_pp + 0.01)
            } else if lower_rank == higher_rank {
                // both entries match the given rank exactly

                Ok(higher_pp + 0.01)
            } else {
                // lerp

                let percent = (lower_rank - rank) as f32 / (lower_rank - higher_rank) as f32;
                let pp = percent * (higher_pp - lower_pp);

                Ok(lower_pp + pp)
            }
        } else if let Some(higher_pp) = entries.higher_pp() {
            // only a higher entry was available
            // e.g. given rank is below any stored rank

            Ok(higher_pp)
        } else if entries.lower_rank().is_some() {
            // only a lower entry was available
            // e.g. given rank is 1 but there was no entry for mode's #1

            Ok(entries.lower_pp())
        } else {
            Ok(0.0)
        }
    }
}

struct DbEntry {
    global_rank: Option<i32>,
    pp: Option<f32>,
    pos: Option<i16>,
}

impl DbEntry {
    const HIGHER: i16 = 0;
    const LOWER: i16 = 1;
}

#[derive(Copy, Clone)]
struct Entry {
    pp: f32,
    rank: u32,
}

impl From<DbEntry> for Entry {
    #[inline]
    fn from(entry: DbEntry) -> Self {
        Self {
            pp: entry.pp.unwrap_or(0.0),
            rank: entry.global_rank.map_or(0, |rank| rank as u32),
        }
    }
}

struct Entries {
    higher: Option<Entry>,
    lower: Option<Entry>,
}

impl Entries {
    fn higher_rank(&self) -> u32 {
        self.higher.as_ref().map_or(1, |entry| entry.rank)
    }

    fn lower_rank(&self) -> Option<u32> {
        self.lower.as_ref().map(|entry| entry.rank)
    }

    fn higher_pp(&self) -> Option<f32> {
        self.higher.as_ref().map(|entry| entry.pp)
    }

    fn lower_pp(&self) -> f32 {
        self.lower.as_ref().map_or(0.0, |entry| entry.pp)
    }
}

impl From<Vec<DbEntry>> for Entries {
    #[inline]
    fn from(entries: Vec<DbEntry>) -> Self {
        let mut higher = None;
        let mut lower = None;

        for entry in entries {
            match entry.pos {
                Some(DbEntry::HIGHER) => {
                    let entry = Entry::from(entry);
                    debug!(pp = entry.pp, rank = entry.rank, "higher");
                    higher = Some(entry);
                }
                Some(DbEntry::LOWER) => {
                    let entry = Entry::from(entry);
                    debug!(pp = entry.pp, rank = entry.rank, "lower");
                    lower = Some(entry);
                }
                _ => unreachable!("invalid pos"),
            }
        }

        Self { higher, lower }
    }
}

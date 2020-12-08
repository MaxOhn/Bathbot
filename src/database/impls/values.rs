use crate::{core::Values, BotResult, Database};

use dashmap::DashMap;
use rayon::prelude::*;
use rosu::model::GameMods;
use sqlx::{types::Json, Row};
use std::collections::HashMap;

type ValueResult = BotResult<Values>;

impl Database {
    pub async fn get_mania_stars(&self) -> ValueResult {
        self.get_values("mania_stars").await
    }

    pub async fn get_ctb_stars(&self) -> ValueResult {
        self.get_values("ctb_stars").await
    }

    async fn get_values(&self, table: &str) -> ValueResult {
        let query = format!("SELECT * FROM {}", table);

        let values: BotResult<DashMap<_, _>> = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                let values = serde_json::from_value::<HashMap<GameMods, f32>>(row.get(1))?
                    .into_iter()
                    .map(|(m, v)| (m, (v, false)))
                    .collect();
                Ok((row.get(0), values))
            })
            .collect();

        Ok(values?)
    }

    pub async fn insert_mania_stars(&self, values: &Values) -> BotResult<usize> {
        self.insert_values("mania_stars", values).await
    }

    pub async fn insert_ctb_stars(&self, values: &Values) -> BotResult<usize> {
        self.insert_values("ctb_stars", values).await
    }

    async fn insert_values(&self, table: &str, values: &Values) -> BotResult<usize> {
        let value_iter = values.iter().filter_map(|guard| {
            let mod_map: HashMap<_, _> = guard
                .value()
                .par_iter()
                .filter_map(
                    |(mods, (pp, to_insert))| if *to_insert { Some((*mods, *pp)) } else { None },
                )
                .collect();

            if mod_map.is_empty() {
                None
            } else {
                Some((*guard.key(), mod_map))
            }
        });

        let mut txn = self.pool.begin().await?;
        let mut counter = 0;

        for (map_id, mod_map) in value_iter {
            let query = format!(
                "INSERT INTO {} VALUES ({},$1) ON CONFLICT (beatmap_id) DO UPDATE SET values=$1",
                table, map_id
            );

            sqlx::query(&query)
                .bind(Json(mod_map))
                .execute(&mut *txn)
                .await?;

            counter += 1;
        }

        txn.commit().await?;

        Ok(counter)
    }
}

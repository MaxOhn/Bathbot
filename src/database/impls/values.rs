use crate::{BotResult, Database};

use dashmap::DashMap;
use rosu::models::GameMods;
use sqlx::{types::Json, Row};
use std::collections::HashMap;

type Values = DashMap<u32, HashMap<GameMods, (f32, bool)>>;
type ValueResult = BotResult<Values>;

impl Database {
    pub async fn get_mania_stars(&self) -> ValueResult {
        self.get_values("mania_stars").await
    }

    pub async fn get_mania_pp(&self) -> ValueResult {
        self.get_values("mania_pp").await
    }

    pub async fn get_ctb_stars(&self) -> ValueResult {
        self.get_values("ctb_stars").await
    }

    pub async fn get_ctb_pp(&self) -> ValueResult {
        self.get_values("ctb_pp").await
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

    pub async fn insert_mania_stars(&self, values: &Values) -> BotResult<()> {
        self.insert_values("mania_stars", values).await
    }

    pub async fn insert_mania_pp(&self, values: &Values) -> BotResult<()> {
        self.insert_values("mania_pp", values).await
    }

    pub async fn insert_ctb_stars(&self, values: &Values) -> BotResult<()> {
        self.insert_values("ctb_stars", values).await
    }

    pub async fn insert_ctb_pp(&self, values: &Values) -> BotResult<()> {
        self.insert_values("ctb_pp", values).await
    }

    async fn insert_values(&self, table: &str, values: &Values) -> BotResult<()> {
        values.retain(|_, mod_map| mod_map.values().any(|(_, to_insert)| *to_insert));
        let mut txn = self.pool.begin().await?;
        for guard in values.into_iter() {
            let (map_id, mod_map) = guard.pair();
            let query = format!("UPDATE {} values=$1 WHERE beatmap_id={}", table, *map_id);
            sqlx::query(&query)
                .bind(Json(mod_map))
                .execute(&mut *txn)
                .await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

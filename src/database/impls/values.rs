use crate::{BotResult, Database};

use dashmap::DashMap;
use rosu::models::GameMods;
use std::{collections::HashMap, fmt::Write};

type ValueResult = BotResult<DashMap<u32, HashMap<GameMods, f32>>>;

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
        let mut query = String::with_capacity(14 + table.len());
        let _ = write!(query, "SELECT * FROM {}", table);
        let client = self.pool.get().await?;
        let statement = client.prepare(&query).await?;
        let values: BotResult<DashMap<_, _>> = client
            .query(&statement, &[])
            .await?
            .into_iter()
            .map(|row| Ok((row.get(0), serde_json::from_value(row.get(1))?)))
            .collect();
        Ok(values?)
    }
}

// // ----------------------------------
// // Table: pp_mania_mods / pp_ctb_mods
// // ----------------------------------

// pub async fn get_mod_pp(
//     &self,
//     map_id: u32,
//     mode: GameMode,
//     mut mods: GameMods,
// ) -> BotResult<Option<f32>> {
//     if mods.contains(GameMods::NightCore) {
//         mods.remove(GameMods::NightCore);
//         mods.insert(GameMods::DoubleTime);
//     }
//     let (table, column) = match mode {
//         GameMode::MNA => ("pp_mania_mods", mania_pp_mods_column(mods)?),
//         GameMode::CTB => {
//             let column = ctb_pp_mods_column(mods);
//             if let Some(column) = column {
//                 ("pp_ctb_mods", column)
//             } else {
//                 return Ok(None);
//             }
//         }
//         _ => unreachable!(),
//     };
//     let query = format!("SELECT {} FROM {} WHERE beatmap_id=?", column, table);
//     let pp: (Option<f32>,) = sqlx::query_as(&query)
//         .bind(map_id)
//         .fetch_one(&self.pool)
//         .await?;
//     Ok(pp.0)
// }

// pub async fn insert_pp_map(
//     &self,
//     map_id: u32,
//     mode: GameMode,
//     mut mods: GameMods,
//     pp: f32,
// ) -> BotResult<()> {
//     if mods.contains(GameMods::NightCore) {
//         mods.remove(GameMods::NightCore);
//         mods.insert(GameMods::DoubleTime);
//     }
//     let (table, column) = match mode {
//         GameMode::MNA => ("pp_mania_mods", mania_pp_mods_column(mods)?),
//         GameMode::CTB => {
//             let column = ctb_pp_mods_column(mods);
//             if let Some(column) = column {
//                 ("pp_ctb_mods", column)
//             } else {
//                 return Ok(());
//             }
//         }
//         _ => unreachable!(),
//     };
//     let query = format!(
//         "
// INSERT INTO
// {} (beatmap_id, {col})
// VALUES
// (?,?) ON DUPLICATE KEY
// UPDATE
// {col}=?
// ",
//         table,
//         col = column
//     );
//     sqlx::query(&query)
//         .bind(map_id)
//         .bind(pp)
//         .bind(pp)
//         .execute(&self.pool)
//         .await?;
//     Ok(())
// }

// // ----------------------------------------
// // Table: stars_mania_mods / stars_ctb_mods
// // ----------------------------------------

// pub async fn get_mod_stars(
//     &self,
//     map_id: u32,
//     mode: GameMode,
//     mut mods: GameMods,
// ) -> BotResult<Option<f32>> {
//     if mods.contains(GameMods::NightCore) {
//         mods.remove(GameMods::NightCore);
//         mods.insert(GameMods::DoubleTime);
//     }
//     let (table, column) = match mode {
//         GameMode::MNA => ("stars_mania_mods", mania_stars_mods_column(mods)?),
//         GameMode::CTB => ("stars_ctb_mods", ctb_stars_mods_column(mods)?),
//         _ => unreachable!(),
//     };
//     let query = format!("SELECT {} FROM {} WHERE beatmap_id=?", column, table);
//     let stars: (Option<f32>,) = sqlx::query_as(&query)
//         .bind(map_id)
//         .fetch_one(&self.pool)
//         .await?;
//     Ok(stars.0)
// }

// pub async fn insert_stars_map(
//     &self,
//     map_id: u32,
//     mode: GameMode,
//     mut mods: GameMods,
//     stars: f32,
// ) -> BotResult<()> {
//     let mania_mods = GameMods::DoubleTime | GameMods::HalfTime;
//     let ctb_mods =
//         GameMods::Easy | GameMods::HardRock | GameMods::DoubleTime | GameMods::HalfTime;
//     if (mode == GameMode::MNA && !mods.intersects(mania_mods))
//         || (mode == GameMode::CTB && !mods.intersects(ctb_mods))
//     {
//         return Ok(());
//     } else if mods.contains(GameMods::NightCore) {
//         mods.remove(GameMods::NightCore);
//         mods.insert(GameMods::DoubleTime);
//     }
//     let (table, column) = match mode {
//         GameMode::MNA => ("stars_mania_mods", mania_stars_mods_column(mods)?),
//         GameMode::CTB => ("stars_ctb_mods", ctb_stars_mods_column(mods)?),
//         _ => unreachable!(),
//     };
//     let query = format!(
//         "
// INSERT INTO
// {} (beatmap_id, {col})
// VALUES
// (?,?) ON DUPLICATE KEY
// UPDATE
// {col}=?
// ",
//         table,
//         col = column
//     );
//     sqlx::query(&query)
//         .bind(map_id)
//         .bind(stars)
//         .bind(stars)
//         .execute(&self.pool)
//         .await?;
//     Ok(())
// }
// }

// fn ctb_pp_mods_column(mods: GameMods) -> Option<&'static str> {
// if (mods - GameMods::Perfect).is_empty() {
//     return Some("NM");
// }
// let valid = GameMods::Hidden | GameMods::HardRock | GameMods::DoubleTime;
// let m = match mods & valid {
//     GameMods::Hidden => "HD",
//     GameMods::HardRock => "HR",
//     GameMods::DoubleTime => "DT",
//     m if m == GameMods::Hidden | GameMods::HardRock => "HDHR",
//     m if m == GameMods::Hidden | GameMods::DoubleTime => "HDDT",
//     _ => return None,
// };
// Some(m)
// }

// fn mania_pp_mods_column(mods: GameMods) -> BotResult<&'static str> {
// let valid = GameMods::Easy | GameMods::NoFail | GameMods::DoubleTime | GameMods::HalfTime;
// let m = match mods & valid {
//     GameMods::NoMod => "NM",
//     GameMods::NoFail => "NF",
//     GameMods::Easy => "EZ",
//     GameMods::DoubleTime => "DT",
//     GameMods::HalfTime => "HT",
//     m if m == GameMods::NoFail | GameMods::Easy => "NFEZ",
//     m if m == GameMods::NoFail | GameMods::DoubleTime => "NFDT",
//     m if m == GameMods::Easy | GameMods::DoubleTime => "EZDT",
//     m if m == GameMods::NoFail | GameMods::HalfTime => "NFHT",
//     m if m == GameMods::Easy | GameMods::HalfTime => "EZHT",
//     m if m == GameMods::NoFail | GameMods::Easy | GameMods::DoubleTime => "NFEZDT",
//     m if m == GameMods::NoFail | GameMods::Easy | GameMods::HalfTime => "NFEZHT",
//     _ => bail!("No valid mod combination for mania pp ({})", mods),
// };
// Ok(m)
// }

// fn ctb_stars_mods_column(mods: GameMods) -> BotResult<&'static str> {
// let valid = GameMods::Easy | GameMods::HardRock | GameMods::DoubleTime | GameMods::HalfTime;
// let m = match mods & valid {
//     GameMods::Easy => "EZ",
//     GameMods::HardRock => "HR",
//     GameMods::DoubleTime => "DT",
//     GameMods::HalfTime => "HT",
//     m if m == GameMods::Easy | GameMods::DoubleTime => "EZDT",
//     m if m == GameMods::HardRock | GameMods::DoubleTime => "HRDT",
//     m if m == GameMods::Easy | GameMods::HalfTime => "EZHT",
//     m if m == GameMods::HardRock | GameMods::HalfTime => "HRHT",
//     _ => bail!("No valid mod combination for ctb stars ({})", mods),
// };
// Ok(m)
// }

// fn mania_stars_mods_column(mods: GameMods) -> BotResult<&'static str> {
// let valid = GameMods::DoubleTime | GameMods::HalfTime;
// let m = match mods & valid {
//     GameMods::DoubleTime => "DT",
//     GameMods::HalfTime => "HT",
//     _ => bail!("No valid mod combination for mania stars ({})", mods),
// };
// Ok(m)
// }

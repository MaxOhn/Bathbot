use sqlx::{mysql::MySqlRow, FromRow, Row};
use std::str::FromStr;

#[derive(Debug)]
pub struct Ratios {
    pub name: String,
    pub scores: Vec<i16>,
    pub ratios: Vec<f32>,
    pub misses: Vec<f32>,
}

impl<'c> FromRow<'c, MySqlRow<'c>> for Ratios {
    fn from_row(row: &MySqlRow<'c>) -> Result<Ratios, sqlx::Error> {
        let scores: &str = row.get("scores");
        let ratios: &str = row.get("ratios");
        let misses: &str = row.get("misses");
        Ok(Ratios {
            name: row.get("name"),
            scores: scores
                .split(',')
                .map(|s| i16::from_str(s).unwrap())
                .collect(),
            ratios: ratios
                .split(',')
                .map(|s| f32::from_str(s).unwrap())
                .collect(),
            misses: misses
                .split(',')
                .map(|s| f32::from_str(s).unwrap())
                .collect(),
        })
    }
}

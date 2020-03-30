use super::super::schema::ratio_table;

use diesel::{deserialize::Queryable, mysql::Mysql};
use std::str::FromStr;

#[derive(Debug)]
pub struct Ratios {
    pub name: String,
    pub scores: Vec<i16>,
    pub ratios: Vec<f32>,
    pub misses: Vec<f32>,
}

impl Queryable<ratio_table::SqlType, Mysql> for Ratios {
    type Row = (String, String, String, String);

    fn build(row: Self::Row) -> Self {
        Self {
            name: row.0,
            scores: row
                .1
                .split(',')
                .map(|s| i16::from_str(s).unwrap())
                .collect(),
            ratios: row
                .2
                .split(',')
                .map(|s| f32::from_str(s).unwrap())
                .collect(),
            misses: row
                .3
                .split(',')
                .map(|s| f32::from_str(s).unwrap())
                .collect(),
        }
    }
}

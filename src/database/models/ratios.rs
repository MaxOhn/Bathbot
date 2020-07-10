use sqlx::FromRow;

#[derive(Debug, FromRow)]
pub struct Ratios {
    pub scores: Vec<i16>,
    pub ratios: Vec<f32>,
    pub misses: Vec<f32>,
}

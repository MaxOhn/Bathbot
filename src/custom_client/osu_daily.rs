use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RankPP {
    pub rank: u32,
    pub pp: f32,
}

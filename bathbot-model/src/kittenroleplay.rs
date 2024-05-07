use rosu_v2::model::user::{CountryCode, Username};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::SnipeCountryStatistics;

#[derive(Deserialize)]
pub struct KittenRoleplayCountries {
    pub full: Vec<KittenRoleplayCountry>,
    pub partial: Vec<KittenRoleplayCountry>,
}

#[derive(Deserialize)]
pub struct KittenRoleplayCountry {
    #[serde(rename = "country")]
    pub code: CountryCode,
    pub coverage: f64,
}

#[derive(Deserialize)]
pub struct KittenRoleplayModsCount {
    pub count: u32,
    pub mods: u32,
}

#[derive(Deserialize)]
pub struct KittenRoleplayScore {
    #[serde(with = "super::deser::adjust_acc")]
    pub accuracy: f32,
    pub artist: Box<str>,
    #[serde(rename = "beatmap_id")]
    pub map_id: u32,
    #[serde(with = "super::deser::datetime_rfc2822")]
    pub created_at: OffsetDateTime,
    pub count_miss: u32,
    pub max_combo: u32,
    pub mods: u32,
    pub pp: Option<f32>,
    pub score: u32,
    pub stars: f32,
    pub title: Box<str>,
    pub version: Box<str>,
}

#[derive(Deserialize)]
pub struct KittenRoleplayCountryRankingPlayer {
    pub average_accuracy: f32,
    pub average_pp: Option<f32>,
    pub average_score: f64,
    pub average_stars: f32,
    pub count: u32,
    pub rank: u32,
    pub total_score: u64,
    pub user_id: u32,
    pub username: Username,
    pub weighted_pp: Option<f32>,
}

#[derive(Deserialize)]
pub struct KittenRoleplayCountryStatistics {
    #[serde(with = "super::deser::datetime_rfc2822")]
    pub last_update: OffsetDateTime,
    pub most_gains_count: i32,
    pub most_gains_user_id: u32,
    pub most_gains_username: Username,
    pub most_losses_count: i32,
    pub most_losses_user_id: u32,
    pub most_losses_username: Username,
    #[serde(rename = "played_beatmaps")]
    pub played_maps: u32,
    #[serde(rename = "total_beatmaps")]
    pub total_maps: u32,
    pub unique_players: u32,
}

impl From<KittenRoleplayCountryStatistics> for SnipeCountryStatistics {
    fn from(stats: KittenRoleplayCountryStatistics) -> Self {
        Self {
            total_maps: Some(stats.total_maps),
            unplayed_maps: stats.total_maps - stats.played_maps,
            most_gains_count: stats.most_gains_count,
            most_gains_user_id: stats.most_gains_user_id,
            most_gains_username: stats.most_gains_username,
            most_losses_count: stats.most_losses_count,
            most_losses_user_id: stats.most_losses_user_id,
            most_losses_username: stats.most_losses_username,
        }
    }
}

#[derive(Deserialize)]
pub struct KittenRoleplayPlayerStatistics {
    #[serde(with = "super::deser::adjust_acc")]
    pub average_accuracy: f32,
    pub average_pp: Option<f32>,
    pub average_score: f32,
    pub average_stars: f32,
    pub count: u32,
    pub count_delta: i32,
    pub count_loved: u32,
    pub count_ranked: u32,
    pub country: CountryCode,
    pub rank_average_accuracy: u32,
    pub rank_average_pp: u32,
    pub rank_average_score: u32,
    pub rank_average_stars: u32,
    pub rank_count: u32,
    pub rank_total_score: u32,
    pub rank_weighted_pp: u32,
    pub total_score: u64,
    pub username: Username,
    pub weighted_pp: Option<f32>,
}

#[derive(Deserialize)]
pub struct KittenRoleplayPlayerHistoryEntry {
    pub average_accuracy: f32,
    pub count: u32,
    #[serde(with = "super::deser::datetime_rfc2822")]
    pub date: OffsetDateTime,
    pub total_score: u64,
    pub weighted_pp: Option<f32>,
}

#[derive(Deserialize)]
pub struct KittenRoleplayStarsCount {
    pub count: u32,
    pub stars: u32,
}

#[derive(Deserialize)]
pub struct KittenRoleplaySnipe {
    #[serde(with = "super::deser::adjust_acc")]
    pub accuracy: f32,
    pub artist: Box<str>,
    #[serde(rename = "beatmap_id")]
    pub map_id: u32,
    pub max_combo: u32,
    pub mods: u32,
    pub pp: Option<f32>,
    pub score: u32,
    #[serde(with = "super::deser::datetime_rfc2822")]
    pub sniped_at: OffsetDateTime,
    pub sniper_user_id: u32,
    pub sniper_username: Username,
    pub stars: f32,
    pub title: Box<str>,
    pub version: Box<str>,
    pub victim_user_id: Option<u32>,
    pub victim_username: Option<Username>,
}

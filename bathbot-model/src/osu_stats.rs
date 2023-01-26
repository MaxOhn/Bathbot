use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    str::FromStr,
};

use bathbot_util::osu::ModSelection;
use rosu_v2::prelude::{GameMode, GameMods, Grade, RankStatus, Username};
use serde::{de::Error, Deserialize, Deserializer};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandOption, CreateOption};

use crate::CountryCode;

use super::deser;

#[derive(Debug)]
pub struct OsuStatsPlayer {
    pub user_id: u32,
    pub count: u32,
    pub username: Username,
}

#[derive(Deserialize)]
struct Outer {
    #[serde(rename = "userId")]
    user_id: u32,
    count: String,
    #[serde(rename = "osu_user")]
    user: Inner,
}

#[derive(serde::Deserialize)]
pub struct Inner {
    #[serde(rename = "userName")]
    username: Username,
}

impl<'de> Deserialize<'de> for OsuStatsPlayer {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let helper = Outer::deserialize(d)?;

        Ok(OsuStatsPlayer {
            user_id: helper.user_id,
            count: u32::from_str(&helper.count).map_err(D::Error::custom)?,
            username: helper.user.username,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct OsuStatsScore {
    #[serde(rename = "userId")]
    pub user_id: u32,
    pub position: u32,
    #[serde(rename = "rank")]
    pub grade: Grade,
    pub score: u32,
    #[serde(rename = "maxCombo")]
    pub max_combo: u32,
    #[serde(with = "deser::f32_string")]
    pub accuracy: f32,
    pub count300: u32,
    pub count100: u32,
    pub count50: u32,
    #[serde(rename = "countKatu")]
    pub count_katu: u32,
    #[serde(rename = "countGeki")]
    pub count_geki: u32,
    #[serde(rename = "countMiss")]
    pub count_miss: u32,
    #[serde(rename = "enabledMods", with = "deser::mods_string")]
    pub mods: GameMods,
    #[serde(rename = "playDate", with = "deser::naive_datetime")]
    pub ended_at: OffsetDateTime,
    #[serde(rename = "ppValue")]
    pub pp: Option<f32>,
    #[serde(rename = "beatmap")]
    pub map: OsuStatsMap,
}

#[derive(Debug, Deserialize)]
pub struct OsuStatsMap {
    #[serde(rename = "beatmapId")]
    pub map_id: u32,
    #[serde(rename = "beatmapSetId")]
    pub mapset_id: u32,
    #[serde(rename = "approved")]
    pub status: RankStatus,
    #[serde(rename = "lastUpdated", with = "deser::naive_datetime")]
    pub last_updated: OffsetDateTime,
    #[serde(rename = "approvedDate", with = "deser::option_naive_datetime")]
    pub approved_date: Option<OffsetDateTime>,
    #[serde(rename = "hitLength")]
    pub seconds_drain: u32,
    #[serde(rename = "totalLength")]
    pub seconds_total: u32,
    pub mode: GameMode,
    pub version: String,
    pub artist: String,
    pub title: String,
    pub creator: Username,
    pub bpm: f32,
    pub source: String,
    #[serde(rename = "diffRating", with = "deser::option_f32_string")]
    pub stars: Option<f32>,
    #[serde(rename = "diffSize", with = "deser::f32_string")]
    pub diff_cs: f32,
    #[serde(rename = "diffOverall", with = "deser::f32_string")]
    pub diff_od: f32,
    #[serde(rename = "diffApproach", with = "deser::f32_string")]
    pub diff_ar: f32,
    #[serde(rename = "diffDrain", with = "deser::f32_string")]
    pub diff_hp: f32,
    #[serde(rename = "maxCombo")]
    pub max_combo: Option<u32>,
}

impl Default for OsuStatsScoresOrder {
    #[inline]
    fn default() -> Self {
        Self::Date
    }
}

impl Display for OsuStatsScoresOrder {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}

#[derive(Debug)]
pub struct OsuStatsParams {
    pub username: Username,
    pub mode: GameMode,
    pub page: usize,
    pub min_rank: usize,
    pub max_rank: usize,
    pub min_acc: f32,
    pub max_acc: f32,
    pub order: OsuStatsScoresOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl OsuStatsParams {
    pub fn new(username: impl Into<Username>) -> Self {
        Self {
            username: username.into(),
            mode: GameMode::Osu,
            page: 1,
            min_rank: 1,
            max_rank: 100,
            min_acc: 0.0,
            max_acc: 100.0,
            order: OsuStatsScoresOrder::default(),
            mods: None,
            descending: true,
        }
    }

    pub fn mode(mut self, mode: GameMode) -> Self {
        self.mode = mode;

        self
    }
}

#[derive(Debug)]
pub struct OsuStatsListParams {
    pub country: Option<CountryCode>,
    pub mode: GameMode,
    pub page: usize,
    pub rank_min: usize,
    pub rank_max: usize,
}

#[derive(Copy, Clone, CreateOption, CommandOption, Debug)]
pub enum OsuStatsScoresOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc = 3,
    #[option(name = "Combo", value = "combo")]
    Combo = 4,
    #[option(name = "Date", value = "date")]
    Date = 0,
    #[option(name = "Misses", value = "misses")]
    Misses = 6,
    #[option(name = "PP", value = "pp")]
    Pp = 1,
    #[option(name = "Rank", value = "rank")]
    Rank = 2,
    #[option(name = "Score", value = "score")]
    Score = 5,
}

pub struct OsuStatsPlayersArgs {
    pub mode: GameMode,
    pub country: Option<CountryCode>,
    pub page: usize,
    pub min_rank: u32,
    pub max_rank: u32,
}

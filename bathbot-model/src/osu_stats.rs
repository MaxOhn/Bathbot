use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    str::FromStr,
};

use bathbot_util::{osu::ModSelection, ScoreExt, ScoreHasEndedAt};
use rosu_v2::prelude::{GameMod, GameMode, GameMods, Grade, RankStatus, Username};
use serde::{
    de::{
        value::StrDeserializer, DeserializeSeed, Error as DeError, IgnoredAny, SeqAccess,
        Unexpected, Visitor,
    },
    Deserialize, Deserializer,
};
use serde_json::value::RawValue;
use time::OffsetDateTime;
use twilight_interactions::command::{CommandOption, CreateOption};

use super::deser;
use crate::CountryCode;

#[derive(Debug)]
pub struct OsuStatsPlayer {
    pub user_id: u32,
    pub count: u32,
    pub username: Username,
}

impl<'de> Deserialize<'de> for OsuStatsPlayer {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        pub struct Inner {
            #[serde(rename = "userName")]
            username: Username,
        }

        #[derive(Deserialize)]
        struct Outer<'a> {
            #[serde(rename = "userId")]
            user_id: u32,
            count: &'a str,
            #[serde(rename = "osu_user")]
            user: Inner,
        }

        let helper = Outer::deserialize(d)?;

        Ok(OsuStatsPlayer {
            user_id: helper.user_id,
            count: u32::from_str(helper.count).map_err(D::Error::custom)?,
            username: helper.user.username,
        })
    }
}

#[derive(Debug)]
pub struct OsuStatsScores {
    pub scores: Vec<OsuStatsScore>,
    pub count: usize,
}

pub struct OsuStatsScoresSeed {
    mode: GameMode,
}

impl OsuStatsScoresSeed {
    pub fn new(mode: GameMode) -> Self {
        Self { mode }
    }
}

impl<'de> DeserializeSeed<'de> for OsuStatsScoresSeed {
    type Value = OsuStatsScores;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for OsuStatsScoresSeed {
    type Value = OsuStatsScores;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("an OsuStatsScores sequence")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let scores = seq
            .next_element_seed(OsuStatsScoreVecSeed::new(self.mode))?
            .ok_or_else(|| {
                DeError::custom("OsuStatsScores sequence must contain scores as first element")
            })?;

        let count = seq.next_element()?.ok_or_else(|| {
            DeError::custom("OsuStatsScores sequence must contain count as second element")
        })?;

        while seq.next_element::<IgnoredAny>()?.is_some() {}

        Ok(OsuStatsScores { scores, count })
    }
}

#[derive(Debug)]
pub struct OsuStatsScore {
    pub user_id: u32,
    pub position: u32,
    pub grade: Grade,
    pub score: u32,
    pub max_combo: u32,
    pub accuracy: f32,
    pub count300: u32,
    pub count100: u32,
    pub count50: u32,
    pub count_katu: u32,
    pub count_geki: u32,
    pub count_miss: u32,
    pub mods: GameMods,
    pub ended_at: OffsetDateTime,
    pub pp: Option<f32>,
    pub map: OsuStatsMap,
}

pub struct OsuStatsScoreVecSeed {
    mode: GameMode,
}

impl OsuStatsScoreVecSeed {
    pub fn new(mode: GameMode) -> Self {
        Self { mode }
    }
}

impl<'de> DeserializeSeed<'de> for OsuStatsScoreVecSeed {
    type Value = Vec<OsuStatsScore>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for OsuStatsScoreVecSeed {
    type Value = Vec<OsuStatsScore>;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("a sequence of OsuStatsScore")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        #[derive(Deserialize)]
        struct OsuStatsScoreInner<'mods> {
            #[serde(rename = "userId")]
            user_id: u32,
            position: u32,
            #[serde(rename = "rank")]
            grade: Grade,
            score: u32,
            #[serde(rename = "maxCombo")]
            max_combo: u32,
            #[serde(with = "deser::f32_string")]
            accuracy: f32,
            count300: u32,
            count100: u32,
            count50: u32,
            #[serde(rename = "countKatu")]
            count_katu: u32,
            #[serde(rename = "countGeki")]
            count_geki: u32,
            #[serde(rename = "countMiss")]
            count_miss: u32,
            #[serde(borrow, rename = "enabledMods")]
            mods: &'mods RawValue,
            #[serde(rename = "playDate", with = "deser::naive_datetime")]
            ended_at: OffsetDateTime,
            #[serde(rename = "ppValue")]
            pp: Option<f32>,
            #[serde(rename = "beatmap")]
            map: OsuStatsMap,
        }

        let mut scores = Vec::with_capacity(seq.size_hint().unwrap_or(0));

        while let Some(inner) = seq.next_element::<OsuStatsScoreInner<'de>>()? {
            let score = OsuStatsScore {
                mods: StrDeserializer::new(inner.mods.get().trim_matches('"'))
                    .deserialize_str(OsuStatsScoreModsVisitor { mode: self.mode })?,
                user_id: inner.user_id,
                position: inner.position,
                grade: inner.grade,
                score: inner.score,
                max_combo: inner.max_combo,
                accuracy: inner.accuracy,
                count300: inner.count300,
                count100: inner.count100,
                count50: inner.count50,
                count_katu: inner.count_katu,
                count_geki: inner.count_geki,
                count_miss: inner.count_miss,
                ended_at: inner.ended_at,
                pp: inner.pp,
                map: inner.map,
            };

            scores.push(score);
        }

        Ok(scores)
    }
}

struct OsuStatsScoreModsVisitor {
    mode: GameMode,
}

impl<'de> Visitor<'de> for OsuStatsScoreModsVisitor {
    type Value = GameMods;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("GameMods")
    }

    fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
        if v == "None" {
            return Ok(GameMods::new());
        }

        v.split(',')
            .map(|s| GameMod::new(s, self.mode))
            .collect::<Option<GameMods>>()
            .ok_or_else(|| {
                DeError::invalid_value(Unexpected::Str(v), &"comma separated list of acronyms")
            })
    }
}

#[rustfmt::skip]
impl ScoreExt for OsuStatsScore {
    #[inline] fn count_miss(&self) -> u32 { self.count_miss }
    #[inline] fn count_50(&self) -> u32 { self.count50 }
    #[inline] fn count_100(&self) -> u32 { self.count100 }
    #[inline] fn count_300(&self) -> u32 { self.count300 }
    #[inline] fn count_geki(&self) -> u32 { self.count_geki }
    #[inline] fn count_katu(&self) -> u32 { self.count_katu }
    #[inline] fn max_combo(&self) -> u32 { self.max_combo }
    #[inline] fn mods(&self) -> &GameMods { &self.mods }
    #[inline] fn grade(&self, _: GameMode) -> Grade { self.grade }
    #[inline] fn score(&self) -> u32 { self.score }
    #[inline] fn pp(&self) -> Option<f32> { self.pp }
    #[inline] fn accuracy(&self) -> f32 { self.accuracy }
}

#[rustfmt::skip]
impl ScoreHasEndedAt for OsuStatsScore {
    #[inline] fn ended_at(&self) -> OffsetDateTime { self.ended_at }
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
    pub version: Box<str>,
    pub artist: Box<str>,
    pub title: Box<str>,
    pub creator: Username,
    pub bpm: f32,
    // pub source: Box<str>,
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

use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    str::FromStr,
};

use bathbot_util::{osu::ModSelection, ScoreExt, ScoreHasEndedAt};
use eyre::{Result, WrapErr};
use rosu_v2::prelude::{
    CountryCode, GameMod, GameModIntermode, GameMode, GameMods, Grade, RankStatus, Username,
};
use serde::{
    de::{
        value::StrDeserializer, DeserializeSeed, Error as DeError, IgnoredAny, SeqAccess, Visitor,
    },
    Deserialize, Deserializer,
};
use serde_json::value::RawValue;
use time::{Date, OffsetDateTime};
use twilight_interactions::command::{CommandOption, CreateOption};

use super::deser;
use crate::{
    deser::ModeAsSeed,
    rkyv_util::time::{DateRkyv, DateTimeRkyv},
    rosu_v2::grade::GradeRkyv,
};

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

/// Wrapper to avoid premature deserialization of the scores if only the count
/// is needed
pub struct OsuStatsScoresRaw {
    mode: GameMode,
    bytes: Vec<u8>,
}

impl OsuStatsScoresRaw {
    pub fn new(mode: GameMode, bytes: Vec<u8>) -> Self {
        Self { mode, bytes }
    }

    pub fn count(&self) -> Result<usize> {
        let count_res = self
            .bytes
            .rsplit(|byte| *byte == b',')
            .nth(2)
            .map(|bytes| std::str::from_utf8(bytes).map(str::parse));

        if let Some(Ok(Ok(count))) = count_res {
            Ok(count)
        } else {
            let body = String::from_utf8_lossy(&self.bytes);

            eyre::bail!("Failed to deserialize count of osustats: {body}")
        }
    }

    pub fn into_scores(self) -> Result<OsuStatsScores> {
        let mut deserializer = serde_json::Deserializer::from_slice(&self.bytes);

        self.deserialize(&mut deserializer).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&self.bytes);

            format!("Failed to deserialize osustats global: {body}")
        })
    }
}

#[derive(Debug)]
pub struct OsuStatsScores {
    pub scores: Vec<OsuStatsScore>,
    pub count: usize,
}

impl<'de> DeserializeSeed<'de> for &OsuStatsScoresRaw {
    type Value = OsuStatsScores;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for &OsuStatsScoresRaw {
    type Value = OsuStatsScores;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("an OsuStatsScores sequence")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let scores = seq
            .next_element_seed(ModeAsSeed::<Vec<OsuStatsScore>>::new(self.mode))?
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

impl<'de> DeserializeSeed<'de> for ModeAsSeed<Vec<OsuStatsScore>> {
    type Value = Vec<OsuStatsScore>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for ModeAsSeed<Vec<OsuStatsScore>> {
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
            let mods = StrDeserializer::new(inner.mods.get().trim_matches('"'))
                .deserialize_str(self.cast::<GameMods>())?;

            let score = OsuStatsScore {
                mods,
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

impl<'de> Visitor<'de> for ModeAsSeed<GameMods> {
    type Value = GameMods;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("GameMods")
    }

    fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
        if v == "None" {
            return Ok(GameMods::new());
        }

        fn parse_mod_alt(s: &str) -> Option<GameMod> {
            match s {
                "K1" => Some(GameMod::OneKeyMania(Default::default())),
                "K2" => Some(GameMod::TwoKeysMania(Default::default())),
                "K3" => Some(GameMod::ThreeKeysMania(Default::default())),
                "K4" => Some(GameMod::FourKeysMania(Default::default())),
                "K5" => Some(GameMod::FiveKeysMania(Default::default())),
                "K6" => Some(GameMod::SixKeysMania(Default::default())),
                "K7" => Some(GameMod::SevenKeysMania(Default::default())),
                "K8" => Some(GameMod::EightKeysMania(Default::default())),
                "K9" => Some(GameMod::NineKeysMania(Default::default())),
                _ => None,
            }
        }

        let mut mods = v
            .split(',')
            .map(|s| parse_mod_alt(s).unwrap_or_else(|| GameMod::new(s, self.mode)))
            .collect::<GameMods>();

        // osustats doesn't seem to validate mods so we have to do it
        if mods.contains_intermode(GameModIntermode::Nightcore) {
            mods.remove_intermode(GameModIntermode::DoubleTime);
        }

        if mods.contains_intermode(GameModIntermode::Perfect) {
            mods.remove_intermode(GameModIntermode::SuddenDeath);
        }

        Ok(mods)
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
    #[inline] fn grade(&self) -> Grade { self.grade }
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

#[derive(Clone, Debug)]
pub struct OsuStatsParams {
    pub username: Username,
    pub mode: GameMode,
    pub page: usize,
    pub min_rank: usize,
    pub max_rank: usize,
    pub min_acc: f32,
    pub max_acc: f32,
    pub order: OsuStatsScoresOrder,
    pub descending: bool,
    // private to ensure the `.mods(_)` method is used when building
    mods: Option<ModSelection>,
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

    pub fn mode(&mut self, mode: GameMode) -> &mut Self {
        self.mode = mode;

        self
    }

    pub fn page(&mut self, page: usize) -> &mut Self {
        self.page = page;

        self
    }

    pub fn min_rank(&mut self, min_rank: usize) -> &mut Self {
        self.min_rank = min_rank;

        self
    }

    pub fn max_rank(&mut self, max_rank: usize) -> &mut Self {
        self.max_rank = max_rank;

        self
    }

    pub fn min_acc(&mut self, min_acc: f32) -> &mut Self {
        self.min_acc = min_acc;

        self
    }

    pub fn max_acc(&mut self, max_acc: f32) -> &mut Self {
        self.max_acc = max_acc;

        self
    }

    pub fn order(&mut self, order: OsuStatsScoresOrder) -> &mut Self {
        self.order = order;

        self
    }

    pub fn descending(&mut self, descending: bool) -> &mut Self {
        self.descending = descending;

        self
    }

    pub fn mods(&mut self, mut mods: ModSelection) -> &mut Self {
        if let ModSelection::Exact(ref mut mods) = mods {
            // if NC is set, DT must be set too
            if mods.contains(GameModIntermode::Nightcore) {
                mods.insert(GameModIntermode::DoubleTime);
            }

            // if PF is set, SD must be set too
            if mods.contains(GameModIntermode::Perfect) {
                mods.insert(GameModIntermode::SuddenDeath);
            }
        }

        self.mods = Some(mods);

        self
    }

    pub fn get_mods(&self) -> Option<&ModSelection> {
        self.mods.as_ref()
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

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum OsuStatsBestTimeframe {
    #[option(name = "Yesterday", value = "yesterday")]
    Yesterday = 1,
    #[option(name = "Last Week", value = "week")]
    LastWeek = 2,
    #[option(name = "Last Month", value = "month")]
    LastMonth = 3,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct OsuStatsBestScores {
    #[with(DateRkyv)]
    pub start_date: Date,
    #[with(DateRkyv)]
    pub end_date: Date,
    pub scores: Box<[OsuStatsBestScore]>,
}

impl<'de> DeserializeSeed<'de> for ModeAsSeed<OsuStatsBestScores> {
    type Value = OsuStatsBestScores;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for ModeAsSeed<OsuStatsBestScores> {
    type Value = OsuStatsBestScores;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("a sequence with two entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        #[derive(Deserialize)]
        struct OsuStatsBestDates {
            #[serde(with = "deser::date")]
            start: Date,
            #[serde(with = "deser::date")]
            end: Date,
        }

        let Some(OsuStatsBestDates { start, end }) = seq.next_element()? else {
            return Err(DeError::custom(
                "first entry of sequence must contain start and end date",
            ));
        };

        let Some(scores) = seq.next_element_seed(self.cast::<Vec<OsuStatsBestScore>>())? else {
            return Err(DeError::custom(
                "second entry of sequence must be list of recentbest scores",
            ));
        };

        Ok(OsuStatsBestScores {
            start_date: start,
            end_date: end,
            scores: scores.into_boxed_slice(),
        })
    }
}

impl<'de> DeserializeSeed<'de> for ModeAsSeed<Vec<OsuStatsBestScore>> {
    type Value = Vec<OsuStatsBestScore>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for ModeAsSeed<Vec<OsuStatsBestScore>> {
    type Value = Vec<OsuStatsBestScore>;

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("a sequence of recent best scores")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut scores = Vec::with_capacity(seq.size_hint().unwrap_or(0));

        while let Some(score) = seq.next_element_seed(self.cast::<OsuStatsBestScore>())? {
            scores.push(score);
        }

        Ok(scores)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct OsuStatsBestScore {
    pub accuracy: f32,
    pub count_miss: u32,
    pub mods: GameMods,
    pub max_combo: u32,
    #[with(DateTimeRkyv)]
    pub ended_at: OffsetDateTime,
    pub position: u32,
    pub pp: f32,
    #[with(GradeRkyv)]
    pub grade: Grade,
    pub score: u32,
    pub map: OsuStatsBestScoreMap,
    pub user: OsuStatsBestScoreUser,
}

impl<'de> DeserializeSeed<'de> for ModeAsSeed<OsuStatsBestScore> {
    type Value = OsuStatsBestScore;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        #[derive(Deserialize)]
        struct OsuStatsBestScoreInner<'a> {
            #[serde(with = "deser::f32_string")]
            accuracy: f32,
            #[serde(rename = "countMiss")]
            count_miss: u32,
            #[serde(borrow, rename = "enabledMods")]
            mods: &'a RawValue,
            #[serde(rename = "maxCombo")]
            max_combo: u32,
            #[serde(rename = "playDate", with = "deser::naive_datetime")]
            ended_at: OffsetDateTime,
            position: u32,
            #[serde(rename = "ppValue")]
            pp: f32,
            #[serde(rename = "rank")]
            grade: Grade,
            score: u32,
            #[serde(rename = "beatmap")]
            map: OsuStatsBestScoreMap,
            #[serde(rename = "osu_user")]
            user: OsuStatsBestScoreUser,
        }

        let score = OsuStatsBestScoreInner::deserialize(d)?;

        let mods = StrDeserializer::new(score.mods.get().trim_matches('"'))
            .deserialize_str(self.cast::<GameMods>())?;

        let score = OsuStatsBestScore {
            accuracy: score.accuracy,
            count_miss: score.count_miss,
            mods,
            max_combo: score.max_combo,
            ended_at: score.ended_at,
            position: score.position,
            pp: score.pp,
            grade: score.grade,
            score: score.score,
            map: score.map,
            user: score.user,
        };

        Ok(score)
    }
}

#[derive(Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct OsuStatsBestScoreMap {
    #[serde(rename = "beatmapId")]
    pub map_id: u32,
    #[serde(rename = "beatmapSetId")]
    pub mapset_id: u32,
    pub artist: Box<str>,
    pub title: Box<str>,
    pub version: Box<str>,
    pub creator: Box<str>,
    #[serde(rename = "maxCombo")]
    pub max_combo: u32,
}

#[derive(Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct OsuStatsBestScoreUser {
    #[serde(rename = "userId")]
    pub user_id: u32,
    #[serde(rename = "userName")]
    pub username: Box<str>,
}

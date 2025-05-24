use std::fmt;

use rkyv::rancor::BoxedError;
use serde::{Deserialize, de};
use time::OffsetDateTime;

use crate::{
    deser::{Datetime, datetime_rfc3339, u64_string},
    rkyv_util::time::DateTimeRkyv,
};

#[derive(Clone)]
pub struct RankAccPeaks {
    pub rank: u32,
    pub rank_timestamp: OffsetDateTime,
    pub acc: f64,
    pub acc_timestamp: OffsetDateTime,
}

impl RankAccPeaks {
    pub fn deserialize(bytes: &[u8]) -> Result<Option<Self>, serde_json::Error> {
        let MaybeRankAccPeaks(peaks) = serde_json::from_slice(bytes)?;

        Ok(peaks)
    }
}

struct MaybeRankAccPeaks(Option<RankAccPeaks>);

impl<'de> de::Deserialize<'de> for MaybeRankAccPeaks {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct RankAccPeaksVisitor;

        impl<'de> de::Visitor<'de> for RankAccPeaksVisitor {
            type Value = MaybeRankAccPeaks;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a RankAccPeaks object")
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let peaks = match seq.next_element::<Self::Value>()? {
                    Some(peaks) => peaks,
                    None => MaybeRankAccPeaks(None),
                };

                Ok(peaks)
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut rank = None;
                let mut rank_timestamp = None;
                let mut acc = None;
                let mut acc_timestamp = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "best_global_rank" => rank = map.next_value()?,
                        "best_rank_timestamp" => rank_timestamp = map.next_value()?,
                        "best_accuracy" => acc = map.next_value()?,
                        "best_acc_timestamp" => acc_timestamp = map.next_value()?,
                        _ => {
                            return Err(de::Error::invalid_value(
                                de::Unexpected::Str(key),
                                &"best_global_rank, best_rank_timestamp, \
                                best_accuracy, best_acc_timestamp",
                            ));
                        }
                    }
                }

                let (
                    Some(rank),
                    Some(Datetime(rank_timestamp)),
                    Some(acc),
                    Some(Datetime(acc_timestamp)),
                ) = (rank, rank_timestamp, acc, acc_timestamp)
                else {
                    return Ok(MaybeRankAccPeaks(None));
                };

                Ok(MaybeRankAccPeaks(Some(RankAccPeaks {
                    rank,
                    rank_timestamp,
                    acc,
                    acc_timestamp,
                })))
            }
        }

        d.deserialize_any(RankAccPeaksVisitor)
    }
}

#[derive(Deserialize, rkyv::Archive, rkyv::Serialize)]
pub struct OsuTrackHistoryEntry {
    pub count300: u64,
    pub count100: u64,
    pub count50: u64,
    pub playcount: u32,
    #[serde(with = "u64_string")]
    pub ranked_score: u64,
    #[serde(with = "u64_string")]
    pub total_score: u64,
    pub pp_rank: u32,
    pub level: f32,
    #[serde(rename = "pp_raw")]
    pub pp: f32,
    pub accuracy: f32,
    #[serde(rename = "count_rank_ss")]
    pub count_ss: i32,
    #[serde(rename = "count_rank_s")]
    pub count_s: i32,
    #[serde(rename = "count_rank_a")]
    pub count_a: i32,
    #[serde(with = "datetime_rfc3339")]
    #[rkyv(with = DateTimeRkyv)]
    pub timestamp: OffsetDateTime,
}

impl ArchivedOsuTrackHistoryEntry {
    pub fn ratio_count300(&self) -> f32 {
        100.0 * self.count300.to_native() as f32 / self.sum_counts() as f32
    }

    pub fn ratio_count100(&self) -> f32 {
        100.0 * self.count100.to_native() as f32 / self.sum_counts() as f32
    }

    pub fn ratio_count50(&self) -> f32 {
        100.0 * self.count50.to_native() as f32 / self.sum_counts() as f32
    }

    fn sum_counts(&self) -> u64 {
        self.count300 + self.count100 + self.count50
    }

    pub fn timestamp(&self) -> OffsetDateTime {
        self.timestamp.try_deserialize::<BoxedError>().unwrap()
    }
}

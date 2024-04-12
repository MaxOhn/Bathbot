use std::fmt;

use serde::de;
use time::OffsetDateTime;

use crate::deser::Datetime;

#[derive(Clone)]
pub struct RankAccPeaks {
    pub rank: u32,
    pub rank_timestamp: OffsetDateTime,
    pub acc: f64,
    pub acc_timestamp: OffsetDateTime,
}

impl RankAccPeaks {
    pub fn deserialize(bytes: &[u8]) -> Result<Option<Self>, serde_json::Error> {
        struct RankAccPeaksVisitor;

        impl<'de> de::Visitor<'de> for RankAccPeaksVisitor {
            type Value = Option<RankAccPeaks>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a RankAccPeaks object")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut rank = None;
                let mut rank_timestamp = None;
                let mut acc = None;
                let mut acc_timestamp = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "best_global_rank" => rank = map.next_value()?,
                        "best_global_rank_timestamp" => rank_timestamp = map.next_value()?,
                        "best_accuracy" => acc = map.next_value()?,
                        "best_accuracy_timestamp" => acc_timestamp = map.next_value()?,
                        _ => {
                            return Err(de::Error::invalid_value(
                                de::Unexpected::Str(key),
                                &"best_global_rank, best_global_rank_timestamp, \
                                best_accuracy, best_accuracy_timestamp",
                            ))
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
                    return Ok(None);
                };

                Ok(Some(RankAccPeaks {
                    rank,
                    rank_timestamp,
                    acc,
                    acc_timestamp,
                }))
            }
        }

        let mut d = serde_json::Deserializer::from_slice(bytes);

        <_ as de::Deserializer<'_>>::deserialize_map(&mut d, RankAccPeaksVisitor)
    }
}

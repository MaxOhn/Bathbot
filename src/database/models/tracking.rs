use chrono::{DateTime, Utc};
use rosu_v2::model::GameMode;
use serde_json::Value;
use sqlx::{types::Json, ColumnIndex, Decode, Error, FromRow, Row, Type};
use std::{collections::HashMap, str::FromStr};
use twilight_model::id::ChannelId;

#[derive(Debug)]
pub struct TrackingUser {
    pub user_id: u32,
    pub mode: GameMode,
    pub last_top_score: DateTime<Utc>,
    pub channels: HashMap<ChannelId, usize>,
}

impl TrackingUser {
    #[inline]
    pub fn new(
        user_id: u32,
        mode: GameMode,
        last_top_score: DateTime<Utc>,
        channel: ChannelId,
        limit: usize,
    ) -> Self {
        let mut channels = HashMap::with_capacity(1);
        channels.insert(channel, limit);

        Self {
            user_id,
            mode,
            last_top_score,
            channels,
        }
    }

    #[inline]
    pub fn remove_channel(&mut self, channel: ChannelId) -> bool {
        self.channels.remove(&channel).is_some()
    }
}

impl<'r, R> FromRow<'r, R> for TrackingUser
where
    R: Row,
    usize: ColumnIndex<R>,
    i8: Type<<R as Row>::Database>,
    i8: Decode<'r, <R as Row>::Database>,
    u32: Type<<R as Row>::Database>,
    u32: Decode<'r, <R as Row>::Database>,
    DateTime<Utc>: Type<<R as Row>::Database>,
    DateTime<Utc>: Decode<'r, <R as Row>::Database>,
    Json<Value>: Type<<R as Row>::Database>,
    Json<Value>: Decode<'r, <R as Row>::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, Error> {
        let user_id: u32 = row.try_get(0)?;
        let mode: i8 = row.try_get(1)?;
        let mode = GameMode::from(mode as u8);
        let last_top_score: DateTime<Utc> = row.try_get(2)?;

        let channels = match serde_json::from_value::<HashMap<String, usize>>(row.try_get(3)?) {
            Ok(channels) => channels
                .into_iter()
                .map(|(id, limit)| (ChannelId(u64::from_str(&id).unwrap()), limit))
                .collect(),
            Err(why) => {
                unwind_error!(
                    warn,
                    why,
                    "Could not deserialize tracking channels value for ({},{}): {}",
                    user_id,
                    mode
                );

                HashMap::new()
            }
        };

        Ok(Self {
            user_id,
            mode,
            last_top_score,
            channels,
        })
    }
}

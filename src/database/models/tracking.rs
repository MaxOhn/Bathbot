use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::model::GameMode;
use serde_json::Value;
use sqlx::{types::Json, ColumnIndex, Decode, Error, FromRow, Row, Type};
use std::collections::HashMap as StdHashMap;
use twilight_model::id::{marker::ChannelMarker, Id};

#[derive(Debug)]
pub struct TrackingUser {
    pub user_id: u32,
    pub mode: GameMode,
    pub last_top_score: DateTime<Utc>,
    pub channels: HashMap<Id<ChannelMarker>, usize>,
}

impl TrackingUser {
    pub fn new(
        user_id: u32,
        mode: GameMode,
        last_top_score: DateTime<Utc>,
        channel: Id<ChannelMarker>,
        limit: usize,
    ) -> Self {
        let mut channels = HashMap::new();
        channels.insert(channel, limit);

        Self {
            user_id,
            mode,
            last_top_score,
            channels,
        }
    }

    pub fn remove_channel(&mut self, channel: Id<ChannelMarker>) -> bool {
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

        let channels = match serde_json::from_value::<StdHashMap<String, usize>>(row.try_get(3)?) {
            Ok(channels) => channels
                .into_iter()
                .map(|(id, limit)| (Id::new(id.parse().unwrap()), limit))
                .collect(),
            Err(why) => {
                let wrap =
                    format!("failed to deserialize tracking channels value for ({user_id},{mode})");
                let report = Report::new(why).wrap_err(wrap);
                warn!("{:?}", report);

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

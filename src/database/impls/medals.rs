use crate::{
    database::{DBOsuMedal, OsuMedal},
    BotResult, Database,
};

use futures::stream::StreamExt;
use hashbrown::HashMap;
use std::fmt::Write;

pub enum MedalResult {
    None,
    Single(OsuMedal),
    Multi(Vec<OsuMedal>),
}

impl Database {
    pub async fn get_medals(&self) -> BotResult<HashMap<u32, OsuMedal>> {
        let mut results = sqlx::query_as!(DBOsuMedal, "SELECT * FROM medals").fetch(&self.pool);
        let mut medals = HashMap::with_capacity(257);

        while let Some(medal) = results.next().await.transpose()? {
            medals.insert(medal.medal_id as u32, medal.into());
        }

        Ok(medals)
    }

    #[allow(dead_code)]
    pub async fn get_medals_name(&self, name: &str) -> BotResult<MedalResult> {
        let mut pattern = String::with_capacity(name.len() + 2);
        let _ = write!(pattern, "%{}%", name);

        let mut results = sqlx::query_as!(
            DBOsuMedal,
            "SELECT * FROM medals WHERE name ILIKE $1",
            pattern
        )
        .fetch(&self.pool);

        let mut medals = Vec::new();

        while let Some(medal) = results.next().await.transpose()? {
            medals.push(medal.into());
        }

        let result = if medals.len() > 1 {
            MedalResult::Multi(medals)
        } else if let Some(medal) = medals.pop() {
            MedalResult::Single(medal)
        } else {
            MedalResult::None
        };

        Ok(result)
    }
}

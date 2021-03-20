use crate::{
    database::{DBOsuMedal, OsuMedal},
    BotResult, Database,
};

use futures::stream::StreamExt;
use std::collections::HashMap;

impl Database {
    pub async fn get_medals(&self) -> BotResult<HashMap<u32, OsuMedal>> {
        let mut results = sqlx::query_as!(DBOsuMedal, "SELECT * FROM medals").fetch(&self.pool);
        let mut medals = HashMap::with_capacity(257);

        while let Some(medal) = results.next().await.transpose()? {
            medals.insert(medal.medal_id as u32, medal.into());
        }

        Ok(medals)
    }
}

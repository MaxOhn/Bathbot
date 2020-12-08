use crate::{BotResult, Database};

use dashmap::DashMap;
use rosu::model::GameMods;
use std::collections::HashMap;

pub type Values = DashMap<u32, HashMap<GameMods, (f32, bool)>>;

pub struct StoredValues {
    pub mania_stars: Values,
    pub ctb_stars: Values,
}

impl StoredValues {
    pub async fn new(psql: &Database) -> BotResult<Self> {
        let (mania_stars, ctb_stars) =
            tokio::try_join!(psql.get_mania_stars(), psql.get_ctb_stars(),)?;

        Ok(Self {
            mania_stars,
            ctb_stars,
        })
    }
}

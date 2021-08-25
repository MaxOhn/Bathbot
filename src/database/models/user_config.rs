use crate::{commands::osu::ProfileSize, Name};

use rosu_v2::prelude::GameMode;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, Error, FromRow, Row};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserConfig {
    #[serde(rename = "m")]
    pub mode: GameMode,
    #[serde(default, rename = "n", skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,
    #[serde(rename = "p")]
    pub profile_embed_size: ProfileSize,
    #[serde(rename = "r")]
    pub recent_embed_maximize: bool,
}

impl<'c> FromRow<'c, PgRow> for UserConfig {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        serde_json::from_value(row.get("config")).map_err(|why| Error::Decode(Box::new(why)))
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {
            mode: GameMode::STD,
            name: None,
            profile_embed_size: ProfileSize::Compact,
            recent_embed_maximize: true,
        }
    }
}

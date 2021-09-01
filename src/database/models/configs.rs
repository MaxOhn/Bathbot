use crate::{commands::osu::ProfileSize, Name};

use rosu_v2::prelude::GameMode;
use serde::{Deserialize, Serialize};
use smallstr::SmallString;
use smallvec::SmallVec;
use sqlx::{postgres::PgRow, Error, FromRow, Row};

pub type Prefix = SmallString<[u8; 2]>;
pub type Prefixes = SmallVec<[Prefix; 5]>;
pub type Authorities = SmallVec<[u64; 4]>;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GuildConfig {
    #[serde(rename = "l", alias = "with_lyrics")]
    pub with_lyrics: bool,
    #[serde(rename = "p", alias = "prefixes")]
    pub prefixes: Prefixes,
    #[serde(rename = "a", alias = "authorities")]
    pub authorities: Authorities,
}

impl<'c> FromRow<'c, PgRow> for GuildConfig {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        serde_json::from_value(row.get("config")).map_err(|why| Error::Decode(Box::new(why)))
    }
}

impl Default for GuildConfig {
    fn default() -> Self {
        GuildConfig {
            with_lyrics: true,
            prefixes: smallvec!["<".into()],
            authorities: smallvec![],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserConfig {
    #[serde(default = "get_true", rename = "r", skip_serializing_if = "is_true")]
    pub embeds_maximized: bool,
    #[serde(default, rename = "m", skip_serializing_if = "Option::is_none")]
    pub mode: Option<GameMode>,
    #[serde(default, rename = "n", skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,
    #[serde(default, rename = "p", skip_serializing_if = "Option::is_none")]
    pub profile_size: Option<ProfileSize>,
    #[serde(default = "get_true", rename = "s", skip_serializing_if = "is_true")]
    pub show_retries: bool,
}

impl UserConfig {
    /// If given mode is not STD, overwrite it with config mode.
    /// Otherwise return given mode.
    pub fn mode(&self, mode: GameMode) -> GameMode {
        match (mode, self.mode) {
            (GameMode::STD, Some(mode_)) => mode_,
            _ => mode,
        }
    }
}

impl<'c> FromRow<'c, PgRow> for UserConfig {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        serde_json::from_value(row.get("config")).map_err(|why| Error::Decode(Box::new(why)))
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {
            embeds_maximized: true,
            mode: None,
            name: None,
            profile_size: None,
            show_retries: true,
        }
    }
}

fn is_true(b: &bool) -> bool {
    *b
}

fn get_true() -> bool {
    true
}

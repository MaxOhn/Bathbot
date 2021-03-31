use serde::{Deserialize, Serialize};
use smallstr::SmallString;
use smallvec::SmallVec;
use sqlx::{postgres::PgRow, Error, FromRow, Row};

pub type Prefix = SmallString<[u8; 2]>;
pub type Prefixes = SmallVec<[Prefix; 5]>;
pub type Authorities = SmallVec<[u64; 4]>;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GuildConfig {
    pub with_lyrics: bool,
    pub prefixes: Prefixes,
    pub authorities: Authorities,
    #[serde(default, skip_serializing)]
    pub modified: bool,
}

impl<'c> FromRow<'c, PgRow> for GuildConfig {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        serde_json::from_value(row.get("config")).map_err(|why| Error::Decode(Box::new(why)))
    }
}

impl Default for GuildConfig {
    #[inline]
    fn default() -> Self {
        GuildConfig {
            with_lyrics: true,
            prefixes: smallvec!["<".into()],
            authorities: smallvec![],
            modified: true,
        }
    }
}
